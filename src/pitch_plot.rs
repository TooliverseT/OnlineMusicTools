use log::info;
use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use std::collections::VecDeque;
use std::f64::consts::LOG10_E;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PitchPlotProps {
    pub current_freq: f64,
    pub history: VecDeque<(f64, f64)>, // (timestamp, frequency)
}

#[function_component(PitchPlot)]
pub fn pitch_plot(props: &PitchPlotProps) -> Html {
    let canvas_ref = use_node_ref();
    let last_center_midi = use_state(|| 69); // MIDI 69 (A4)를 기본값으로 설정

    // 드래그 관련 상태 추가
    let is_dragging = use_state(|| false);
    let drag_start_x = use_state(|| 0);
    let drag_start_y = use_state(|| 0);
    let view_offset_x = use_state(|| 0.0); // 시간축 오프셋 (초)
    let view_offset_y = use_state(|| 0.0); // MIDI 노트 오프셋
    let auto_follow = use_state(|| true); // 자동 따라가기 모드 (기본값: 활성화)

    // 고정 시간 범위를 위한 상태 추가
    let fixed_time_range = use_state(|| None::<(f64, f64)>); // 고정된 시간 범위 (시작, 끝)

    // 마우스 이벤트 핸들러
    let on_mouse_down = {
        let is_dragging = is_dragging.clone();
        let drag_start_x = drag_start_x.clone();
        let drag_start_y = drag_start_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_range = fixed_time_range.clone();
        let history = props.history.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(true);
            drag_start_x.set(e.client_x());
            drag_start_y.set(e.client_y());

            // 드래그 시작시 자동 따라가기 비활성화
            if *auto_follow {
                auto_follow.set(false);

                // 현재 보고 있는 시간 범위를 고정
                let window_duration = 30.0;
                let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                let x_min = if history_duration < window_duration {
                    0.0
                } else {
                    history_duration - window_duration
                };

                let x_max = x_min + window_duration;
                fixed_time_range.set(Some((x_min, x_max)));
            }
        })
    };

    let on_mouse_move = {
        let is_dragging = is_dragging.clone();
        let drag_start_x = drag_start_x.clone();
        let drag_start_y = drag_start_y.clone();
        let view_offset_x = view_offset_x.clone();
        let view_offset_y = view_offset_y.clone();
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();
        let fixed_time_range = fixed_time_range.clone();

        Callback::from(move |e: MouseEvent| {
            if !*is_dragging {
                return;
            }

            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                let canvas_width = canvas.width() as i32;
                let canvas_height = canvas.height() as i32;

                // X축 이동 (시간)
                let dx = e.client_x() - *drag_start_x;
                let window_duration = 30.0;
                let time_per_pixel = window_duration / canvas_width as f64;
                let dt = -dx as f64 * time_per_pixel;

                // 현재 고정된 시간 범위가 있다면 그것을 기준으로 이동
                if let Some((current_min, current_max)) = *fixed_time_range {
                    let new_min = current_min + dt;
                    let new_max = current_max + dt;

                    // 최대 히스토리 길이를 넘어서지 않도록 제한
                    let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);
                    if new_max <= history_duration && new_min >= 0.0 {
                        fixed_time_range.set(Some((new_min, new_max)));
                    }
                }

                // Y축 이동 (주파수 스케일) - 여전히 MIDI 단위로 내부 계산
                let dy = e.client_y() - *drag_start_y;
                let midi_range = 10.0; // 화면에 표시되는 MIDI 노트 범위의 대략적인 값
                let midi_per_pixel = midi_range / canvas_height as f64;
                let dmidi = -dy as f64 * midi_per_pixel;

                // 새로운 Y 오프셋 (음높이) - MIDI 기준으로 유지
                let new_offset_y = *view_offset_y + dmidi;

                // MIDI 범위: 0-127 내에서만 이동 가능하도록 제한
                let midi_range_half = 5.0;
                let min_offset_y = -69.0 + midi_range_half; // C-1(0)에 도달하는 제한
                let max_offset_y = 58.0 - midi_range_half; // G9(127)에 도달하는 제한

                view_offset_y.set(new_offset_y.max(min_offset_y).min(max_offset_y));

                // 드래그 시작점 업데이트
                drag_start_x.set(e.client_x());
                drag_start_y.set(e.client_y());
            }
        })
    };

    let on_mouse_up = {
        let is_dragging = is_dragging.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            is_dragging.set(false);
        })
    };

    let on_double_click = {
        let view_offset_x = view_offset_x.clone();
        let view_offset_y = view_offset_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_range = fixed_time_range.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            // 더블 클릭 시 원래 위치로 리셋
            view_offset_x.set(0.0);
            view_offset_y.set(0.0);
            auto_follow.set(true); // 자동 따라가기 다시 활성화
            fixed_time_range.set(None); // 고정 시간 범위 해제
        })
    };

    {
        let canvas_ref = canvas_ref.clone();
        let history = props.history.clone();
        let current_freq = props.current_freq;
        let last_center_midi_handle = last_center_midi.clone();
        let view_offset_y = view_offset_y.clone();
        let auto_follow = auto_follow.clone();
        let fixed_time_range = fixed_time_range.clone();

        use_effect_with(
            (
                history.clone(),
                current_freq,
                *view_offset_y,
                *auto_follow,
                fixed_time_range.clone(),
            ),
            move |_| {
                if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                    let backend = CanvasBackend::with_canvas_object(canvas).unwrap();
                    let root = backend.into_drawing_area();
                    root.fill(&WHITE).unwrap();

                    let (_width, height) = root.dim_in_pixel();

                    // 시간 범위 계산
                    let window_duration = 30.0;
                    let history_duration = history.back().map(|(t, _)| *t).unwrap_or(0.0);

                    // 고정 모드 또는 자동 모드에 따라 x축 범위 계산
                    let (x_min, x_max) = if let Some((min, max)) = *fixed_time_range {
                        // 고정 모드: 사용자가 드래그한 범위 사용
                        (min, max)
                    } else if *auto_follow {
                        // 자동 모드: 최신 데이터 표시
                        if history_duration < window_duration {
                            (0.0, window_duration)
                        } else {
                            (history_duration - window_duration, history_duration)
                        }
                    } else {
                        // 예전 방식 (기존 코드 호환성)
                        if history_duration < window_duration {
                            (0.0, window_duration)
                        } else {
                            (history_duration - window_duration, history_duration)
                        }
                    };

                    // 현재 중심 주파수 계산
                    let center_freq = if current_freq <= 0.0 {
                        // 주파수가 0이면 마지막 저장된 MIDI 값으로부터 주파수 계산
                        freq_from_midi(*last_center_midi_handle)
                    } else {
                        // 현재 주파수 사용
                        if *auto_follow {
                            last_center_midi_handle.set(midi_from_freq(current_freq));
                            // MIDI 값 업데이트 (참조용)
                        }
                        current_freq
                    };

                    // Y축 오프셋 적용 (Hz 단위)
                    let adjusted_center_freq = if *auto_follow {
                        center_freq
                    } else {
                        // view_offset_y는 MIDI 단위이므로 주파수로 변환해서 적용
                        let midi_offset = midi_from_freq(center_freq) as f64 - *view_offset_y;
                        freq_from_midi(midi_offset as i32)
                    };

                    // 주파수 범위 계산 (옥타브 단위로 설정)
                    let freq_range_factor = 2.0; // 중심 주파수의 몇 배까지 표시할지 (1.5 = ±반옥타브)

                    let min_freq = adjusted_center_freq / freq_range_factor;
                    let max_freq = adjusted_center_freq * freq_range_factor;

                    // 참조용: 해당 주파수 범위에 해당하는 MIDI 노트 범위 계산
                    let min_midi = midi_from_freq(min_freq);
                    let max_midi = midi_from_freq(max_freq);

                    let min_log = min_freq.log10();
                    let max_log = max_freq.log10();

                    // Chart 만들기: y축은 주파수(Hz) 값을 사용
                    let mut chart = ChartBuilder::on(&root)
                        .margin(10)
                        .set_label_area_size(LabelAreaPosition::Left, 50)
                        .set_label_area_size(LabelAreaPosition::Bottom, 40)
                        .build_cartesian_2d(x_min..x_max, min_log..max_log) // 로그 스케일 범위 사용
                        .unwrap();

                    // Y축 라벨 설정 - MIDI 노트 값으로 표시
                    // 표시할 MIDI 노트 범위 계산
                    let midi_steps = (max_midi - min_midi + 1).max(1).min(12) as usize; // 최소 1개, 최대 12개

                    chart
                        .configure_mesh()
                        .y_labels(midi_steps)
                        .y_max_light_lines(0)
                        .x_max_light_lines(5)
                        .y_label_formatter(&|log_freq| {
                            let freq = 10f64.powf(*log_freq);
                            let midi = midi_from_freq(freq);
                            note_name_from_midi(midi)
                        })
                        .x_desc("Time (s)")
                        .y_desc("Frequency (Hz)")
                        .draw()
                        .unwrap();

                    // 여러 LineSeries를 연결하여 그리기
                    let mut segments: Vec<Vec<(f64, f64)>> = Vec::new();
                    let mut current_segment: Vec<(f64, f64)> = Vec::new();
                    let mut last_point: Option<(f64, f64)> = None;

                    // 선 자르기(clipping) 함수
                    fn clip_line_to_y_range(
                        p1: (f64, f64),
                        p2: (f64, f64),
                        y_min: f64,
                        y_max: f64,
                    ) -> Option<Vec<(f64, f64)>> {
                        // p1과 p2가 모두 범위 밖에 있고 같은 방향이면 그리지 않음
                        if (p1.1 < y_min && p2.1 < y_min) || (p1.1 > y_max && p2.1 > y_max) {
                            return None;
                        }

                        let mut result = Vec::new();

                        // 양 끝점이 모두 범위 안에 있으면 그대로 반환
                        if p1.1 >= y_min && p1.1 <= y_max && p2.1 >= y_min && p2.1 <= y_max {
                            result.push(p1);
                            result.push(p2);
                            return Some(result);
                        }

                        // 선이 Y축 하한선과 교차하는 지점 계산
                        if (p1.1 < y_min && p2.1 >= y_min) || (p1.1 >= y_min && p2.1 < y_min) {
                            let t = (y_min - p1.1) / (p2.1 - p1.1);
                            let x = p1.0 + t * (p2.0 - p1.0);
                            result.push((x, y_min));
                        }

                        // 선이 Y축 상한선과 교차하는 지점 계산
                        if (p1.1 > y_max && p2.1 <= y_max) || (p1.1 <= y_max && p2.1 > y_max) {
                            let t = (y_max - p1.1) / (p2.1 - p1.1);
                            let x = p1.0 + t * (p2.0 - p1.0);
                            result.push((x, y_max));
                        }

                        // 범위 내에 있는 점 추가
                        if p1.1 >= y_min && p1.1 <= y_max {
                            result.push(p1);
                        }
                        if p2.1 >= y_min && p2.1 <= y_max {
                            result.push(p2);
                        }

                        // 결과가 비어있으면 None 반환
                        if result.is_empty() {
                            None
                        } else {
                            // x 좌표로 정렬하여 올바른 순서 보장
                            result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                            Some(result)
                        }
                    }

                    for (t, freq) in history.iter() {
                        if *t < x_min || *t > x_max {
                            // 시간 범위 밖이면 현재 세그먼트 종료
                            if !current_segment.is_empty() {
                                segments.push(current_segment.clone());
                                current_segment = Vec::new();
                            }
                            last_point = None;
                            continue;
                        }

                        if *freq == 0.0 {
                            // 주파수가 0이면 현재 세그먼트 종료
                            if !current_segment.is_empty() {
                                segments.push(current_segment.clone());
                                current_segment = Vec::new();
                            }
                            last_point = None;
                            continue;
                        }

                        let log_freq = freq.log10();
                        let current_point = (*t, log_freq);

                        // 이전 점과 현재 점을 연결
                        if let Some(prev_point) = last_point {
                            // 둘 중 하나 이상이 범위를 벗어난 경우 선 자르기 적용
                            if log_freq < min_log
                                || log_freq > max_log
                                || prev_point.1 < min_log
                                || prev_point.1 > max_log
                            {
                                if let Some(clipped_points) = clip_line_to_y_range(
                                    prev_point,
                                    current_point,
                                    min_log,
                                    max_log,
                                ) {
                                    // 잘린 선분이 있으면 추가
                                    if clipped_points.len() >= 2 {
                                        // 현재 세그먼트가 비어있지 않고 첫 점이 이전 세그먼트의 마지막 점과 다르면 새 세그먼트 시작
                                        if !current_segment.is_empty()
                                            && (current_segment.last().unwrap().0
                                                != clipped_points[0].0
                                                || current_segment.last().unwrap().1
                                                    != clipped_points[0].1)
                                        {
                                            segments.push(current_segment.clone());
                                            current_segment = Vec::new();
                                        }

                                        for p in clipped_points {
                                            current_segment.push(p);
                                        }
                                    }
                                } else {
                                    // 잘린 선분이 없으면 현재 세그먼트 종료
                                    if !current_segment.is_empty() {
                                        segments.push(current_segment.clone());
                                        current_segment = Vec::new();
                                    }
                                }
                            } else {
                                // 두 점 모두 범위 내에 있는 경우
                                if current_segment.is_empty() {
                                    current_segment.push(prev_point);
                                }
                                current_segment.push(current_point);
                            }
                        } else if log_freq >= min_log && log_freq <= max_log {
                            // 첫 점이고 범위 내에 있으면 세그먼트 시작
                            current_segment.push(current_point);
                        }

                        last_point = Some(current_point);
                    }

                    // 마지막 세그먼트 추가
                    if !current_segment.is_empty() {
                        segments.push(current_segment);
                    }

                    // 모든 세그먼트 그리기
                    for segment in segments {
                        if segment.len() >= 2 {
                            chart.draw_series(LineSeries::new(segment, &RED)).unwrap();
                        }
                    }

                    // 현재 모드 표시 (드래그 모드 또는 자동 모드)
                    if !*auto_follow {
                        let style = TextStyle::from(("sans-serif", 15).into_font()).color(&BLUE);
                        chart
                            .draw_series(std::iter::once(Text::new(
                                "Drag Mode (Double-click to reset)",
                                (x_min + 0.5, max_log - 0.05),
                                &style,
                            )))
                            .unwrap();

                        // 고정된 시간 범위 정보 표시
                        // if let Some((min, max)) = *fixed_time_range {
                        //     let time_info = format!("Time: {:.1}s - {:.1}s", min, max);
                        //     chart
                        //         .draw_series(std::iter::once(Text::new(
                        //             time_info,
                        //             (x_min + 0.5, max_log - 0.15),
                        //             &style,
                        //         )))
                        //         .unwrap();
                        // }
                    }
                }

                || ()
            },
        );
    }

    html! {
        <canvas
            ref={canvas_ref}
            width=800
            height=400
            onmousedown={on_mouse_down}
            onmousemove={on_mouse_move}
            onmouseup={&on_mouse_up}
            onmouseleave={on_mouse_up.clone()}
            ondblclick={on_double_click}
            style="cursor: move;"
        />
    }
}

// MIDI 관련 함수
fn midi_from_freq(freq: f64) -> i32 {
    (12.0 * (freq / 440.0).log2() + 69.0).round() as i32
}

fn midi_float_from_freq(freq: f64) -> f64 {
    12.0 * (freq / 440.0).log2() + 69.0
}

fn freq_from_midi(midi: i32) -> f64 {
    440.0 * 2f64.powf((midi as f64 - 69.0) / 12.0)
}

fn note_name_from_midi(midi: i32) -> String {
    let notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let note = notes[((midi % 12 + 12) % 12) as usize];
    let octave = midi / 12 - 1;
    format!("{}{}", note, octave)
}
