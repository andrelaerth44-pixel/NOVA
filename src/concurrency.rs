//! Concurrency primitives for the NOVA runtime and tools.
use crate::Value;
use std::{collections::{HashMap,VecDeque},process::{Child,Command,Stdio},sync::{atomic::{AtomicU64,Ordering},Mutex,OnceLock},thread};

fn processes()->&'static Mutex<HashMap<u64,Child>>{static S:OnceLock<Mutex<HashMap<u64,Child>>>=OnceLock::new();S.get_or_init(||Mutex::new(HashMap::new()))}
fn channels()->&'static Mutex<HashMap<u64,VecDeque<String>>> { static S:OnceLock<Mutex<HashMap<u64,VecDeque<String>>>>=OnceLock::new(); S.get_or_init(||Mutex::new(HashMap::new())) }
fn encode(value:&Value)->Result<String,String>{match value{Value::Num(v)=>Ok(format!("N:{}",v)),Value::Str(v)=>Ok(format!("S:{}",v)),Value::Bool(v)=>Ok(format!("B:{}",v)),Value::Null=>Ok("Z:".into()),_=>Err("channels support only number/string/bool/null".into())}}
fn decode(text:&str)->Result<Value,String>{let (kind,payload)=text.split_once(':').unwrap_or((text,""));match kind{"N"=>payload.parse::<f64>().map(Value::Num).map_err(|e|e.to_string()),"S"=>Ok(Value::Str(payload.to_string())),"B"=>Ok(Value::Bool(payload=="true")),"Z"=>Ok(Value::Null),_=>Err("invalid channel message".into())}}
static NEXT:AtomicU64=AtomicU64::new(1);

pub fn spawn_nova(path:&str)->Result<u64,String>{
    let exe=std::env::current_exe().map_err(|e|e.to_string())?;
    let child=Command::new(exe).args(["run",path]).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().map_err(|e|format!("spawn: {}",e))?;
    let id=NEXT.fetch_add(1,Ordering::Relaxed);
    processes().lock().map_err(|_|"process table poisoned")?.insert(id,child);
    Ok(id)
}
pub fn join_nova(id:u64)->Result<i32,String>{
    let mut child=processes().lock().map_err(|_|"process table poisoned")?.remove(&id).ok_or_else(||format!("unknown process {}",id))?;
    Ok(child.wait().map_err(|e|e.to_string())?.code().unwrap_or(-1))
}
pub fn channel()->u64{
    let id=NEXT.fetch_add(1,Ordering::Relaxed);
    channels().lock().expect("channel table poisoned").insert(id,VecDeque::new());
    id
}
pub fn send(id:u64,value:Value)->Result<(),String>{
    channels().lock().map_err(|_|"channel table poisoned")?.get_mut(&id).ok_or_else(||format!("unknown channel {}",id))?.push_back(encode(&value)?);Ok(())
}
pub fn recv(id:u64)->Result<Option<Value>,String>{
    let value=channels().lock().map_err(|_|"channel table poisoned")?.get_mut(&id).ok_or_else(||format!("unknown channel {}",id))?.pop_front();
    value.map(|v|decode(&v)).transpose()
}
pub fn parallel_sum(values:&[f64],workers:usize)->f64{
    if values.is_empty(){return 0.0;}
    let workers=workers.max(1).min(values.len());let chunk=(values.len()+workers-1)/workers;
    thread::scope(|scope|{
        let handles=values.chunks(chunk).map(|part|scope.spawn(move||part.iter().sum::<f64>())).collect::<Vec<_>>();
        handles.into_iter().map(|h|h.join().unwrap_or(0.0)).sum()
    })
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn channel_roundtrip(){let id=channel();send(id,Value::Num(42.0)).unwrap();assert!(matches!(recv(id).unwrap(),Some(Value::Num(42.0))));}
 #[test]fn sum(){assert_eq!(parallel_sum(&[1.0,2.0,3.0,4.0],2),10.0);}
}
