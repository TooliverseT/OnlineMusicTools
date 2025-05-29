use web_sys::HtmlCanvasElement;
use web_sys::HtmlInputElement;
use wasm_bindgen::JsCast;
use std::collections::VecDeque;
use yew::prelude::*;

// 진폭 시각화를 위한 Props 정의
#[derive(Properties, PartialEq)]
pub struct AmplitudeVisualizerProps {
    pub amplitude_data: Option<Vec<f32>>, // 진폭 데이터 배열
    pub sample_rate: Option<f64>,         // 샘플링 레이트
    pub is_recording: bool,               // 녹음 중인지 여부
    pub is_playing: bool,                 // 재생 중인지 여부
    pub history: Option<VecDeque<(f64, Vec<f32>)>>, // 진폭 히스토리 (시간, 진폭 데이터 배열)
}

// 진폭 시각화 컴포넌트 정의
#[function_component(AmplitudeVisualizer)]
pub fn amplitude_visualizer(props: &AmplitudeVisualizerProps) -> Html {
    // 캔버스 참조 생성
    let canvas_ref = use_node_ref();
    
    // 진폭 그래프 렌더링
    {
        let canvas_ref = canvas_ref.clone();
        let amplitude_data = props.amplitude_data.clone();
        let is_recording = props.is_recording;
        let is_playing = props.is_playing;
        let history = props.history.clone();
        
        use_effect_with(
            (
                amplitude_data.clone(),
                is_recording,
                is_playing,
                history.clone(),
            ),
            move |_| {
                // 캔버스 요소 가져오기
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    let ctx = canvas
                        .get_context("2d")
                        .unwrap()
                        .unwrap()
                        .dyn_into::<web_sys::CanvasRenderingContext2d>()
                        .unwrap();
                    
                    // 캔버스 크기 설정
                    let width = canvas.width() as f64;
                    let height = canvas.height() as f64;
                    
                    // 배경 그리기
                    ctx.set_fill_style(&"#001117".into());
                    ctx.fill_rect(0.0, 0.0, width, height);
                    
                    // 그리드 그리기 (분홍색)
                    ctx.set_stroke_style(&"#505050".into());
                    ctx.set_line_width(1.0);
                    
                    // 수평 그리드 선
                    let grid_count_y = 10;
                    for i in 0..=grid_count_y {
                        let y = (i as f64 * height) / grid_count_y as f64;
                        ctx.begin_path();
                        ctx.move_to(0.0, y);
                        ctx.line_to(width, y);
                        ctx.stroke();
                    }
                    
                    // 수직 그리드 선
                    let grid_count_x = 20;
                    for i in 0..=grid_count_x {
                        let x = (i as f64 * width) / grid_count_x as f64;
                        ctx.begin_path();
                        ctx.move_to(x, 0.0);
                        ctx.line_to(x, height);
                        ctx.stroke();
                    }
                    
                    // 색상 고정 - #9EF5CF (민트 그린)
                    let primary_color = "#9EF5CF";
                    
                    // 진폭 데이터가 있으면 시각화
                    if let Some(amplitude_data) = amplitude_data {
                        if !amplitude_data.is_empty() {
                            // 막대 그래프 형태로 시각화 (고정)
                            let bar_width = width / amplitude_data.len() as f64;
                            let max_amp = amplitude_data.iter().fold(0.1f32, |a, b| a.max(b.abs()));
                            
                            ctx.set_fill_style(&primary_color.into());
                            
                            for (i, &amp) in amplitude_data.iter().enumerate() {
                                let normalized_amp = (amp.abs() / max_amp) as f64;
                                let bar_height = normalized_amp * height / 2.0;
                                let x = i as f64 * bar_width;
                                let y = height / 2.0 - bar_height;
                                
                                ctx.fill_rect(x, y, bar_width - 1.0, bar_height * 2.0);
                            }
                        }
                    } else if let Some(history) = history {
                        // 진폭 히스토리를 사용한 시각화 (시간에 따른 진폭 데이터)
                        if !history.is_empty() {
                            ctx.set_fill_style(&primary_color.into());
                            
                            let bar_count = width.min(128.0) as usize;
                            let bar_width = width / bar_count as f64;
                            
                            // 가장 최근 bar_count 개의 데이터 포인트만 시각화
                            let history_vec: Vec<(f64, Vec<f32>)> = history.iter().cloned().collect();
                            let start_idx = history_vec.len().saturating_sub(bar_count);
                            let visible_history = &history_vec[start_idx..];
                            
                            for (i, (_, amp_data)) in visible_history.iter().enumerate() {
                                if amp_data.is_empty() {
                                    continue;
                                }
                                
                                // 각 시간 지점에서의 진폭 데이터 배열에서 RMS 값 계산
                                let rms = (amp_data.iter().map(|&x| x * x).sum::<f32>() / amp_data.len() as f32).sqrt();
                                
                                // RMS 값으로 막대 그래프 그리기
                                let bar_height = (rms * height as f32) as f64;
                                let x = i as f64 * bar_width;
                                let y = height - bar_height;
                                
                                ctx.fill_rect(x, y, bar_width - 1.0, bar_height);
                            }
                        }
                    } else {
                        // 데이터가 없는 경우 안내 메시지 표시
                        ctx.set_fill_style(&primary_color.into());
                        ctx.set_font("20px sans-serif");
                        ctx.set_text_align("center");
                        ctx.set_text_baseline("middle");
                        ctx.fill_text("마이크를 활성화하여 진폭을 측정하세요", width / 2.0, height / 2.0).unwrap();
                    }
                }
                
                || () // cleanup 함수
            },
        );
    }
    
    // HTML 렌더링
    html! {
        <div class="amplitude-visualizer">
            <div class="canvas-container">
                <canvas ref={canvas_ref} width="800" height="400" />
            </div>
        </div>
    }
} 