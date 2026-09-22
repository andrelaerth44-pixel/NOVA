use std::{fs,process::Command,path::Path};

pub fn svg_rect(w:u32,h:u32,x:u32,y:u32,rw:u32,rh:u32)->String{
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></svg>\n",w,h,x,y,rw,rh)
}
pub fn write_svg(path:&Path,svg:&str)->Result<(),String>{fs::write(path,svg).map_err(|e|e.to_string())}

pub fn generate_test_tone(rate:u32,seconds:f32,hz:f32)->Vec<i16>{
    let n=(rate as f32*seconds.max(0.0)) as usize;
    (0..n).map(|i|{let t=i as f32/rate as f32;(0.25*i16::MAX as f32*(std::f32::consts::TAU*hz*t).sin()) as i16}).collect()
}
pub fn write_wav(path:&Path,rate:u32,samples:&[i16])->Result<(),String>{
    let data_len=(samples.len()*2) as u32;let mut out=Vec::with_capacity(44+samples.len()*2);
    out.extend_from_slice(b"RIFF");out.extend_from_slice(&(36+data_len).to_le_bytes());out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());out.extend_from_slice(&1u16.to_le_bytes());out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());out.extend_from_slice(&(rate*2).to_le_bytes());out.extend_from_slice(&2u16.to_le_bytes());out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples{out.extend_from_slice(&s.to_le_bytes());}
    fs::write(path,out).map_err(|e|e.to_string())
}
pub fn write_html_app(path:&Path,title:&str,body:&str)->Result<(),String>{
    let html=format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title></head><body>{}</body></html>",title,body);
    fs::write(path,html).map_err(|e|e.to_string())
}
pub fn encode_video_with_ffmpeg(pattern:&str,output:&Path)->Result<(),String>{
    let status=Command::new("ffmpeg").args(["-y","-framerate","30","-i",pattern,"-pix_fmt","yuv420p"]).arg(output).status().map_err(|e|e.to_string())?;
    if status.success(){Ok(())}else{Err(format!("ffmpeg exited with {}",status))}
}
