use std::time::SystemTime;

struct Speed{
    bytes:u64,
    time:u64,
    speed:f32,
}

impl Speed{
    fn new(bytes:u64, time:u64) -> Self{
        let speed = bytes as f32 / time as f32;
        Speed{bytes, time, speed}
    }
}

struct SpeedMonitor{
    last_time:SystemTime,
    last_bytes:u64,
}

impl SpeedMonitor{
    fn new() -> Self{
        SpeedMonitor{last_time:SystemTime::now(), last_bytes:0}
    }
    fn update(&mut self, bytes:u64) -> Speed{
        let time = SystemTime::now();
        let speed = Speed::new(bytes - self.last_bytes, time.duration_since(self.last_time).unwrap().as_millis() as u64);
        self.last_bytes = bytes;
        self.last_time = time;
        speed
    }    
}
