//! Dependency-free tensor and neural-network engine.

#[derive(Clone, Debug, PartialEq)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}
impl Tensor {
    pub fn new(shape: Vec<usize>, data: Vec<f32>) -> Result<Self, String> {
        let expected: usize = shape.iter().product();
        if expected != data.len() { return Err(format!("tensor shape {:?} requires {}, got {}", shape, expected, data.len())); }
        Ok(Self { shape, data })
    }
    pub fn zeros(shape: &[usize]) -> Self { Self { shape: shape.to_vec(), data: vec![0.0; shape.iter().product()] } }
    pub fn map(&self, f: impl Fn(f32) -> f32) -> Self { Self { shape: self.shape.clone(), data: self.data.iter().copied().map(f).collect() } }
    pub fn add(&self, other: &Self) -> Result<Self, String> {
        if self.shape != other.shape { return Err("tensor add shape mismatch".into()); }
        Ok(Self { shape: self.shape.clone(), data: self.data.iter().zip(&other.data).map(|(a,b)| a+b).collect() })
    }
    pub fn matmul(&self, other: &Self) -> Result<Self, String> {
        if self.shape.len()!=2 || other.shape.len()!=2 { return Err("matmul requires rank-2 tensors".into()); }
        let (m,k)=(self.shape[0],self.shape[1]);
        let (k2,n)=(other.shape[0],other.shape[1]);
        if k!=k2 { return Err(format!("matmul inner dimensions differ: {} vs {}",k,k2)); }
        let mut out=vec![0.0;m*n];
        for i in 0..m { for p in 0..k { let a=self.data[i*k+p]; for j in 0..n { out[i*n+j]+=a*other.data[p*n+j]; } } }
        Ok(Self { shape: vec![m,n], data: out })
    }
    pub fn relu(&self) -> Self { self.map(|v| v.max(0.0)) }
    pub fn sigmoid(&self) -> Self { self.map(|v| 1.0/(1.0+(-v).exp())) }
    pub fn mse(&self, target:&Self)->Result<f32,String>{
        if self.shape!=target.shape { return Err("mse shape mismatch".into()); }
        if self.data.is_empty(){return Ok(0.0);}
        Ok(self.data.iter().zip(&target.data).map(|(a,b)|{let d=a-b;d*d}).sum::<f32>()/self.data.len() as f32)
    }
}

#[derive(Clone, Debug)]
pub struct Dense { pub weights: Tensor, pub bias: Tensor }
impl Dense {
    pub fn new(input:usize,output:usize,seed:u64)->Self{
        let mut state=seed|1;
        let mut next=||{state^=state<<13;state^=state>>7;state^=state<<17;((state as f32/u64::MAX as f32)*2.0-1.0)*0.5};
        Self{
            weights:Tensor{shape:vec![input,output],data:(0..input*output).map(|_|next()).collect()},
            bias:Tensor::zeros(&[1,output]),
        }
    }
    pub fn forward(&self,input:&Tensor)->Result<Tensor,String>{input.matmul(&self.weights)?.add(&self.bias)}
}

#[derive(Clone, Debug)]
pub struct Mlp { pub hidden:Dense, pub output:Dense }
impl Mlp {
    pub fn new(input:usize,hidden:usize,output:usize)->Self{
        Self{hidden:Dense::new(input,hidden,0x12345678),output:Dense::new(hidden,output,0x87654321)}
    }
    pub fn forward(&self,input:&Tensor)->Result<(Tensor,Tensor),String>{
        let hidden=self.hidden.forward(input)?.relu();
        let output=self.output.forward(&hidden)?.sigmoid();
        Ok((hidden,output))
    }
    pub fn train_sample(&mut self,input:&[f32],target:&[f32],lr:f32)->Result<f32,String>{
        let x=Tensor::new(vec![1,input.len()],input.to_vec())?;
        let (hidden,pred)=self.forward(&x)?;
        if pred.data.len()!=target.len(){return Err("training target size mismatch".into());}
        let mut delta=vec![0.0;target.len()];
        for i in 0..target.len(){let p=pred.data[i];delta[i]=(p-target[i])*p*(1.0-p);}
        let hw=self.hidden.weights.shape[1]; let iw=self.hidden.weights.shape[0];
        for h in 0..hw{for o in 0..target.len(){self.output.weights.data[h*target.len()+o]-=lr*hidden.data[h]*delta[o];}}
        for o in 0..target.len(){self.output.bias.data[o]-=lr*delta[o];}
        let mut hd=vec![0.0;hw];
        for h in 0..hw{if hidden.data[h]>0.0{for o in 0..target.len(){hd[h]+=delta[o]*self.output.weights.data[h*target.len()+o];}}}
        for i in 0..iw{for h in 0..hw{self.hidden.weights.data[i*hw+h]-=lr*input[i]*hd[h];}}
        for h in 0..hw{self.hidden.bias.data[h]-=lr*hd[h];}
        pred.mse(&Tensor::new(vec![1,target.len()],target.to_vec())?)
    }
    pub fn train_xor(&mut self,epochs:usize,lr:f32)->Result<f32,String>{
        let data=[([0.0,0.0],[0.0]),([0.0,1.0],[1.0]),([1.0,0.0],[1.0]),([1.0,1.0],[0.0])];
        let mut loss=0.0;
        for _ in 0..epochs{loss=0.0;for(x,y)in &data{loss+=self.train_sample(x,y,lr)?;}loss/=data.len() as f32;}
        Ok(loss)
    }
    pub fn predict(&self,input:&[f32])->Result<Vec<f32>,String>{
        let x=Tensor::new(vec![1,input.len()],input.to_vec())?;Ok(self.forward(&x)?.1.data)
    }
}
pub fn train_xor(epochs:usize)->Result<(Mlp,f32),String>{let mut m=Mlp::new(2,8,1);let loss=m.train_xor(epochs,0.8)?;Ok((m,loss))}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn matmul(){let a=Tensor::new(vec![2,2],vec![1.0,2.0,3.0,4.0]).unwrap();let b=Tensor::new(vec![2,1],vec![2.0,1.0]).unwrap();assert_eq!(a.matmul(&b).unwrap().data,vec![4.0,10.0]);}
    #[test]fn xor(){let(m,l)=train_xor(500).unwrap();assert!(l.is_finite());assert!(m.predict(&[0.0,1.0]).unwrap()[0]>0.5);assert!(m.predict(&[0.0,0.0]).unwrap()[0]<0.5);}
}
