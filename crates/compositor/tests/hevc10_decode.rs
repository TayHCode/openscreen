//! Une vidéo HEVC Main 10 (iPhone, #1390) se rend avec ses couleurs, pas en bandes.
//!
//! VideoToolbox sort un `CVPixelBuffer` P010 (16 bits par échantillon) pour un flux 10 bits.
//! Lu comme du NV12 8 bits, chaque échantillon devient deux texels : l'image se couvre de
//! bandes jaunes et vertes. La mire est coupée en deux, rouge à gauche, bleu à droite ;
//! chaque moitié doit garder sa couleur.
//!
//! La CI ne le joue pas : il faut le GPU et un encodeur HEVC 10 bits.
//!
//! ```sh
//! ffmpeg -f lavfi -i "color=c=red:size=640x360:rate=30:duration=1,drawbox=x=320:y=0:w=320:h=360:color=blue:t=fill" \
//!   -vf format=p010le -c:v hevc_videotoolbox -profile:v main10 -tag:v hvc1 split10.mov
//! OPENSCREEN_HEVC10_SOURCE=$PWD/split10.mov cargo test -p openscreen-compositor --test hevc10_decode
//! ```
#![cfg(target_os = "macos")]

use openscreen_compositor::compositor::{live_params_from_scene, Compositor};
use openscreen_compositor::config;
use openscreen_compositor::d3d::Gpu;
use openscreen_compositor::pipeline::ClipSource;
use openscreen_compositor::scene::Scene;
use openscreen_compositor::timeline_walk::render_frames;

fn scene_json(source: &str) -> String {
    format!(
        r##"{{
        "clips": [{{"screenPath":"{source}","webcamPath":"","sourceStartSec":0,"sourceEndSec":1,"webcamOffsetSec":0,"hasAudio":false}}],
        "layout": {{"preset":"no-webcam","webcamSize":1.0,"webcamShape":"rounded","webcamMirror":false,"webcamPosition":null,"webcamReactiveZoom":false}},
        "effects": {{"padding":0,"blur":false,"shadow":0.0,"roundnessFrac":0,"motionBlur":0.0}},
        "background": {{"kind":"color","color":"#303030"}},
        "zoomRegions": [],
        "cursor": {{"show":false,"size":1,"smoothing":0,"motionBlur":0,"clickBounce":0,"clipToBounds":false,"theme":"default"}},
        "cropByClip": [null],
        "output": {{"width":640,"height":360,"fps":null}}
        }}"##
    )
}

#[test]
fn a_10_bit_hevc_frame_keeps_its_colours() {
    let Some(source) = std::env::var_os("OPENSCREEN_HEVC10_SOURCE") else {
        eprintln!("OPENSCREEN_HEVC10_SOURCE absent — test sauté");
        return;
    };
    let source = source.to_string_lossy().replace('\\', "/");
    let gpu = Gpu::create_auto(false).expect("device");
    let clips = [ClipSource {
        screen: source.clone(),
        webcam: String::new(),
        source_start_sec: 0.0,
        source_end_sec: 1.0,
        webcam_offset_sec: 0.0,
        has_audio: false,
    }];
    let mut cfg = config::all().pop().expect("C8");
    cfg.zoom = false;
    cfg.layout_anim = false;
    cfg.mblur_n = 1;
    cfg.bg_blur = 0.0;
    cfg.cursor = false;
    let scene = Scene::from_json(&scene_json(&source)).expect("scène");
    let comp = Compositor::new_sized(&gpu, 640, 360).expect("compositeur");
    comp.set_live_params(live_params_from_scene(&scene));
    comp.set_scene(Some(scene));
    let frames =
        unsafe { render_frames(&clips, &gpu, &comp, &cfg, 30, &[10]) }.expect("render_frames");
    let frame = &frames[0];

    let px = |x: u32, y: u32| {
        let i = ((y * frame.width + x) * 4) as usize;
        [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
    };
    let close =
        |got: [u8; 3], want: [u8; 3]| got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= 40);
    let (w, h) = (frame.width, frame.height);
    for (x, want) in [(w / 4, [255, 0, 0]), (3 * w / 4, [0, 0, 255])] {
        for y in [h / 4, h / 2, 3 * h / 4] {
            let got = px(x, y);
            assert!(close(got, want), "({x},{y}) : {got:?}, attendu ≈ {want:?}");
        }
    }
}
