use serde::Serialize;
use std::{
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, State};
use tungstenite::{accept_hdr, handshake::server::{Request, Response}, Message, WebSocket};

const ARTNET_PORT:u16=6454;
const BRIDGE_WS_PORT:u16=9461;

#[derive(Default)]
struct Bridge {
    running: Mutex<Option<Arc<AtomicBool>>>,
    clients: Arc<Mutex<Vec<WebSocket<TcpStream>>>>,
}

#[derive(Clone,Serialize,Default)]
struct Stats {
    datagrams:u64, accepted:u64, rejected:u64, fps:u32, source:String,
    universe:u16, sequence:u8, last_error:String, bridge_clients:u64,
}

#[derive(Clone,Serialize)]
struct Frame { universe:u16, sequence:u8, source:String, data:Vec<u8> }

fn parse(buf:&[u8])->Result<(u16,u8,Vec<u8>),String>{
    if buf.len()<18{return Err("too short".into())}
    if &buf[0..8]!=b"Art-Net\0"{return Err("bad header".into())}
    if u16::from_le_bytes([buf[8],buf[9]])!=0x5000{return Err("not ArtDMX".into())}
    if u16::from_be_bytes([buf[10],buf[11]])<14{return Err("protocol < 14".into())}
    let len=u16::from_be_bytes([buf[16],buf[17]])as usize;
    if len<2||len>512||len%2!=0{return Err("invalid DMX length".into())}
    if buf.len()<18+len{return Err("truncated".into())}
    let mut data=vec![0u8;512]; data[..len].copy_from_slice(&buf[18..18+len]);
    Ok((u16::from_le_bytes([buf[14],buf[15]])+1,buf[12],data))
}

fn broadcast(clients:&Arc<Mutex<Vec<WebSocket<TcpStream>>>>, frame:&Frame)->u64 {
    let Ok(payload)=serde_json::to_string(&serde_json::json!({"type":"dmx-frame","frame":frame})) else{return 0};
    let mut sent=0;
    if let Ok(mut list)=clients.lock(){
        list.retain_mut(|ws| match ws.send(Message::Text(payload.clone().into())){
            Ok(_)=>{sent+=1;true}
            Err(tungstenite::Error::Io(e)) if e.kind()==std::io::ErrorKind::WouldBlock=>true,
            Err(_)=>false,
        });
    }
    sent
}

fn start_ws_server(clients:Arc<Mutex<Vec<WebSocket<TcpStream>>>>, app:AppHandle){
    std::thread::spawn(move||{
        let listener=match TcpListener::bind(("127.0.0.1",BRIDGE_WS_PORT)){
            Ok(x)=>x,
            Err(e)=>{let _=app.emit("bridge-error",format!("Could not bind VizBridge WebSocket {BRIDGE_WS_PORT}: {e}"));return}
        };
        let _=listener.set_nonblocking(true);
        let _=app.emit("bridge-ws-status","listening");
        loop {
            match listener.accept(){
                Ok((stream,_))=>{
                    match accept_hdr(stream,|req:&Request,mut res:Response|{
                        if req.uri().path()!="/dmx" {
                            *res.status_mut()=tungstenite::http::StatusCode::NOT_FOUND;
                        }
                        Ok(res)
                    }){
                        Ok(ws)=>{
                            let _=ws.get_ref().set_nonblocking(true);
                            if let Ok(mut list)=clients.lock(){list.push(ws)}
                            let _=app.emit("bridge-ws-status","client-connected");
                        }
                        _=>{}
                    }
                }
                Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(25)),
                Err(e)=>{let _=app.emit("bridge-error",e.to_string());std::thread::sleep(Duration::from_millis(100))}
            }
        }
    });
}

#[tauri::command]
fn start_listener(app:AppHandle,state:State<Bridge>)->Result<(),String>{
    let mut g=state.running.lock().map_err(|_|"state lock")?;
    if g.as_ref().is_some_and(|x|x.load(Ordering::Relaxed)){return Ok(())}
    let run=Arc::new(AtomicBool::new(true)); *g=Some(run.clone()); drop(g);
    let clients=state.clients.clone();
    start_ws_server(clients.clone(),app.clone());
    std::thread::spawn(move||{
        let sock=match UdpSocket::bind(("0.0.0.0",ARTNET_PORT)){
            Ok(s)=>s,
            Err(e)=>{let _=app.emit("bridge-stats",Stats{last_error:format!("UDP 6454 bind failed: {e}"),..Default::default()});return}
        };
        let _=sock.set_read_timeout(Some(Duration::from_millis(200)));
        let mut s=Stats{source:"—".into(),universe:1,..Default::default()};
        let mut buf=[0u8;530]; let mut window=Instant::now(); let mut frames=0u32;
        while run.load(Ordering::Relaxed){
            match sock.recv_from(&mut buf){
                Ok((n,src))=>{
                    s.datagrams+=1;s.source=src.to_string();
                    match parse(&buf[..n]){
                        Ok((u,seq,data))=>{
                            s.accepted+=1;s.universe=u;s.sequence=seq;frames+=1;
                            let frame=Frame{universe:u,sequence:seq,source:src.to_string(),data};
                            let _=app.emit("dmx-frame",frame.clone());
                            broadcast(&clients,&frame);
                        },
                        Err(e)=>{s.rejected+=1;s.last_error=e}
                    }
                    if let Ok(list)=clients.lock(){s.bridge_clients=list.len() as u64}
                    let elapsed=window.elapsed();
                    if elapsed>=Duration::from_secs(1){s.fps=(frames as f64/elapsed.as_secs_f64()).round()as u32;frames=0;window=Instant::now()}
                    let _=app.emit("bridge-stats",s.clone());
                },
                Err(e)if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
                Err(e)=>{s.last_error=e.to_string();let _=app.emit("bridge-stats",s.clone());break}
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_listener(state:State<Bridge>)->Result<(),String>{
    if let Some(r)=state.running.lock().map_err(|_|"state lock")?.take(){r.store(false,Ordering::Relaxed)}
    Ok(())
}

fn packet(universe:u16,channel:u16,value:u8)->Vec<u8>{
    let mut p=vec![0u8;18+512];p[..8].copy_from_slice(b"Art-Net\0");
    p[8..10].copy_from_slice(&0x5000u16.to_le_bytes());p[10..12].copy_from_slice(&14u16.to_be_bytes());
    p[12]=1;p[14..16].copy_from_slice(&(universe.saturating_sub(1)).to_le_bytes());p[16..18].copy_from_slice(&512u16.to_be_bytes());
    if(1..=512).contains(&channel){p[17+channel as usize]=value}p
}

#[tauri::command]
fn generate_test_frame(universe:u16,channel:u16,value:u8)->Result<(),String>{
    let sock=UdpSocket::bind(("127.0.0.1",0)).map_err(|e|e.to_string())?;
    sock.send_to(&packet(universe,channel,value),("127.0.0.1",ARTNET_PORT)).map_err(|e|e.to_string())?;Ok(())
}

pub fn run(){
    tauri::Builder::default().manage(Bridge::default())
      .invoke_handler(tauri::generate_handler![start_listener,stop_listener,generate_test_frame])
      .run(tauri::generate_context!()).expect("VizBridge failed")
}