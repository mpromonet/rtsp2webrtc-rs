rtsp2webrtc-rs
===

Rust RTSP client using [retina](https://github.com/scottlamb/retina), WHEP server using [actix_web](https://actix.rs/)
and display using [whep-video-component](https://github.com/Eyevinn/whep-video-component)

Build
---
```
cargo build
```

Test
---
```
cargo run -- --url rtsp://211.132.61.124/axis-media/media.amp
```