use std::{io::{Read,Write},net::TcpStream,process::{Command,Stdio}};

pub fn http_get(url:&str)->Result<String,String>{
    let rest=url.strip_prefix("http://").ok_or("http_get supports http://")?;
    let (authority,path)=rest.split_once('/').map(|(a,p)|(a,format!("/{}",p))).unwrap_or((rest,"/".into()));
    let mut parts=authority.split(':');let host=parts.next().unwrap_or("");
    let port=parts.next().unwrap_or("80").parse::<u16>().map_err(|_|"invalid port")?;
    let mut stream=TcpStream::connect((host,port)).map_err(|e|e.to_string())?;
    stream.write_all(format!("GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: nova\r\n\r\n",path,host).as_bytes()).map_err(|e|e.to_string())?;
    let mut buf=String::new();stream.read_to_string(&mut buf).map_err(|e|e.to_string())?;
    let (head,body)=buf.split_once("\r\n\r\n").unwrap_or((&buf,""));
    if !head.contains(" 200 "){return Err(head.lines().next().unwrap_or("HTTP error").to_string())}Ok(body.into())
}
pub fn run_process(program:&str,args:&[String])->Result<i32,String>{
    Ok(Command::new(program).args(args).status().map_err(|e|e.to_string())?.code().unwrap_or(-1))
}
pub fn capture_process(program:&str,args:&[String])->Result<String,String>{
    let out=Command::new(program).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).output().map_err(|e|e.to_string())?;
    let mut text=String::from_utf8_lossy(&out.stdout).to_string();if !out.status.success(){text.push_str(&String::from_utf8_lossy(&out.stderr));}Ok(text)
}
pub fn sha256_hex(input:&[u8])->String{
    const K:[u32;64]=[
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cbe,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
    ];
    let mut data=input.to_vec(); let bit_len=(data.len() as u64)*8; data.push(0x80);
    while (data.len()+8)%64!=0 { data.push(0); } data.extend_from_slice(&bit_len.to_be_bytes());
    let mut h:[u32;8]=[0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    for chunk in data.chunks_exact(64){
        let mut w=[0u32;64];
        for i in 0..16 { w[i]=u32::from_be_bytes([chunk[i*4],chunk[i*4+1],chunk[i*4+2],chunk[i*4+3]]); }
        for i in 16..64 {
            let s0=w[i-15].rotate_right(7)^w[i-15].rotate_right(18)^(w[i-15]>>3);
            let s1=w[i-2].rotate_right(17)^w[i-2].rotate_right(19)^(w[i-2]>>10);
            w[i]=w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let (mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut hh)=(h[0],h[1],h[2],h[3],h[4],h[5],h[6],h[7]);
        for i in 0..64{
            let s1=e.rotate_right(6)^e.rotate_right(11)^e.rotate_right(25);
            let ch=(e&f)^((!e)&g);
            let t1=hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0=a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22);
            let maj=(a&b)^(a&c)^(b&c); let t2=s0.wrapping_add(maj);
            hh=g;g=f;f=e;e=d.wrapping_add(t1);d=c;c=b;b=a;a=t1.wrapping_add(t2);
        }
        h[0]=h[0].wrapping_add(a);h[1]=h[1].wrapping_add(b);h[2]=h[2].wrapping_add(c);h[3]=h[3].wrapping_add(d);
        h[4]=h[4].wrapping_add(e);h[5]=h[5].wrapping_add(f);h[6]=h[6].wrapping_add(g);h[7]=h[7].wrapping_add(hh);
    }
    h.iter().map(|x|format!("{:08x}",x)).collect()
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn sha256_known(){assert_eq!(sha256_hex(b"abc"),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");}
}
