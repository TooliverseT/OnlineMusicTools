use wasm_bindgen::prelude::*;
use web_sys::{self, CustomEvent, CustomEventInit};
use yew::prelude::*;
use yew_router::prelude::*;
use std::collections::VecDeque;

use crate::PitchAnalyzer;

use log::info;

// 애플리케이션의 라우트 정의
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/pitch-controls")]
    PitchControls,
    #[at("/pitch-plot")]
    PitchPlot,
    #[at("/amplitude-visualizer")]
    AmplitudeVisualizer,
    #[at("/metronome")]
    Metronome,
    #[at("/scale-generator")]
    ScaleGenerator,
    #[at("/piano-keyboard")]
    PianoKeyboard,
    #[not_found]
    #[at("/404")]
    NotFound,
}

// 사이드바 컴포넌트
#[function_component(Sidebar)]
pub fn sidebar() -> Html {
    let current_route = use_route::<Route>().unwrap_or(Route::Home);
    
    html! {
        <div class="sidebar">
            <div class="sidebar-header">
                <div class="logo">
                    <div class="logo-icon">
                        <div class="logo-circle"></div>
                        <div class="logo-circle-overlay"></div>
                    </div>
                    <div class="logo-text">{"MusicalMind"}</div>
                </div>
            </div>
            
            <nav class="sidebar-nav">
                <Link<Route> to={Route::Home} classes={classes!("nav-item", if current_route == Route::Home { "active" } else { "" })}>
                    <span class="nav-icon">{"🏠"}</span>
                    <span class="nav-text">{"Dashboard"}</span>
                </Link<Route>>
                
                <Link<Route> to={Route::PitchPlot} classes={classes!("nav-item", if current_route == Route::PitchPlot { "active" } else { "" })}>
                    <span class="nav-icon">{"📊"}</span>
                    <span class="nav-text">{"Pitch Analyzer"}</span>
                </Link<Route>>
                
                <Link<Route> to={Route::AmplitudeVisualizer} classes={classes!("nav-item", if current_route == Route::AmplitudeVisualizer { "active" } else { "" })}>
                    <span class="nav-icon">{"📈"}</span>
                    <span class="nav-text">{"Amplitude Visualizer"}</span>
                </Link<Route>>
                
                <Link<Route> to={Route::Metronome} classes={classes!("nav-item", if current_route == Route::Metronome { "active" } else { "" })}>
                    <span class="nav-icon">{"🥁"}</span>
                    <span class="nav-text">{"Metronome"}</span>
                </Link<Route>>
                
                <Link<Route> to={Route::ScaleGenerator} classes={classes!("nav-item", if current_route == Route::ScaleGenerator { "active" } else { "" })}>
                    <span class="nav-icon">{"🎵"}</span>
                    <span class="nav-text">{"Scale Generator"}</span>
                </Link<Route>>
                
                <Link<Route> to={Route::PianoKeyboard} classes={classes!("nav-item", if current_route == Route::PianoKeyboard { "active" } else { "" })}>
                    <span class="nav-icon">{"🎹"}</span>
                    <span class="nav-text">{"Piano Keyboard"}</span>
                </Link<Route>>
            </nav>
            
            <div class="sidebar-footer">
                <div class="nav-item logout">
                    <span class="nav-icon">{"👤"}</span>
                    <span class="nav-text">{"Profile"}</span>
                </div>
            </div>
        </div>
    }
}

// 상단 헤더 컴포넌트
#[derive(Properties, PartialEq)]
pub struct TopHeaderProps {
    pub on_mobile_menu_toggle: Callback<()>,
}

#[function_component(TopHeader)]
pub fn top_header(props: &TopHeaderProps) -> Html {
    let current_route = use_route::<Route>().unwrap_or(Route::Home);
    
    let page_title = match current_route {
        Route::Home => "Dashboard",
        Route::PitchPlot => "Pitch Analyzer",
        Route::AmplitudeVisualizer => "Amplitude Visualizer", 
        Route::Metronome => "Metronome",
        Route::ScaleGenerator => "Scale Generator",
        Route::PianoKeyboard => "Piano Keyboard",
        _ => "Dashboard",
    };
    
    let on_menu_click = {
        let on_mobile_menu_toggle = props.on_mobile_menu_toggle.clone();
        Callback::from(move |_| {
            on_mobile_menu_toggle.emit(());
        })
    };
    
    html! {
        <div class="top-header-container">
            <div class="top-header">
                <div class="header-left">
                    <button class="mobile-menu-btn" onclick={on_menu_click}>
                        <span class="hamburger"></span>
                    </button>
                    <h1 class="page-title">{page_title}</h1>
                </div>
                
                <div class="header-right">
                    
                    
                    // 기존 피치 컨트롤 유지
                    <div class="pitch-controls-container">
                        <PitchControls />
                    </div>
                </div>
            </div>
        </div>
    }
}

// 메인 레이아웃 컴포넌트
#[function_component(MainLayout)]
pub fn main_layout() -> Html {
    let route = use_route::<Route>().unwrap_or(Route::Home);
    let is_mobile_menu_open = use_state(|| false);
    
    // 페이지 변경 시 오디오 리소스 정리
    {
        let route = route.clone();
        use_effect_with(
            route,
            move |_| {
                // 페이지 변경 시 PitchAnalyzer 상태 초기화 이벤트 발생
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    // ResetComponent 이벤트 발생 - 컴포넌트 완전 초기화
                    let reset_event = web_sys::Event::new("resetPitchAnalyzer").unwrap();
                    document.dispatch_event(&reset_event).unwrap();
                    
                    // StopAudioResources 이벤트 발생 - 모든 오디오 리소스 정리
                    let stop_resources_event = web_sys::Event::new("stopAudioResources").unwrap();
                    document.dispatch_event(&stop_resources_event).unwrap();
                    
                    web_sys::console::log_1(&format!("페이지 이동 감지: 마이크 비활성화 및 PitchAnalyzer 상태 초기화 이벤트 발생").into());
                }
                
                // 클린업 함수
                || {}
            },
        );
    }

    // 현재 라우트에 따른 컨텐츠 선택
    let content = match route {
        Route::Home => html! { <PitchAnalyzer /> },
        Route::PitchControls => html! { <PitchControlsDetail /> },
        Route::PitchPlot => html! { <PitchPlotDetail /> },
        Route::AmplitudeVisualizer => html! { <AmplitudeVisualizerDetail /> },
        Route::Metronome => html! { <MetronomeDetail /> },
        Route::ScaleGenerator => html! { <ScaleGeneratorDetail /> },
        Route::PianoKeyboard => html! { <PianoKeyboardDetail /> },
        Route::NotFound => html! { <NotFound /> },
    };

    let toggle_mobile_menu = {
        let is_mobile_menu_open = is_mobile_menu_open.clone();
        Callback::from(move |_| {
            is_mobile_menu_open.set(!*is_mobile_menu_open);
        })
    };

    let on_overlay_click = {
        let is_mobile_menu_open = is_mobile_menu_open.clone();
        Callback::from(move |_: MouseEvent| {
            is_mobile_menu_open.set(false);
        })
    };

    html! {
        <div class={classes!("app-layout", if *is_mobile_menu_open { "mobile-menu-open" } else { "" })}>
            <Sidebar />
            <div class="main-content">
                <TopHeader on_mobile_menu_toggle={toggle_mobile_menu.clone()} />
                <main class="content-area">
                    { content }
                </main>
            </div>
            
            // 모바일 오버레이
            if *is_mobile_menu_open {
                <div class="mobile-overlay" onclick={on_overlay_click}></div>
            }
        </div>
    }
}

// 상세 페이지 컴포넌트 - 피치 컨트롤
#[function_component(PitchControlsDetail)]
pub fn pitch_controls_detail() -> Html {
    html! {
        <div class="detail-page">
            <div class="back-link">
                <Link<Route> to={Route::Home}>{"🏠 메인화면으로 돌아가기"}</Link<Route>>
            </div>
            <div class="content full-width">
                <h2>{"피치 컨트롤"}</h2>
                <div class="analyzer-container">
                    <PitchControls />
                </div>
                <div class="description">
                    <h3>{"피치 컨트롤 사용법"}</h3>
                    <p>{"피치 컨트롤은 마이크 입력을 실시간으로 분석하고, 녹음, 재생, 감도 조절, 다운로드 등 다양한 기능을 제공합니다."}</p>
                    <ul>
                        <li>{"🎤 마이크 버튼: 마이크를 켜고 끌 수 있습니다."}</li>
                        <li>{"🔊 모니터 버튼: 입력 소리를 스피커로 직접 들을 수 있습니다."}</li>
                        <li>{"▶️ 재생 버튼: 녹음된 소리를 재생/일시정지합니다."}</li>
                        <li>{"💾 다운로드 버튼: 녹음 파일을 저장할 수 있습니다."}</li>
                        <li>{"🎚️ 감도/스피커 게인: 마이크 감도와 스피커 볼륨을 조절할 수 있습니다."}</li>
                        <li>{"진행 바: 녹음/재생 위치를 확인하고 이동할 수 있습니다."}</li>
                        <li>{"🔗 아이콘: 피치 컨트롤 상세 페이지로 이동하는 링크입니다."}</li>
                    </ul>
                    <p>{"각 버튼에 마우스를 올리면 기능 설명이 툴팁으로 표시됩니다."}</p>
                </div>
            </div>
        </div>
    }
}

// 상세 페이지 컴포넌트 - 피치 플롯
#[function_component(PitchPlotDetail)]
pub fn pitch_plot_detail() -> Html {
    html! {
        <div class="detail-page">
            <div class="back-link">
                <Link<Route> to={Route::Home}>{"🏠 메인화면으로 돌아가기"}</Link<Route>>
            </div>
            <div class="content full-width">
                <h2>{"음높이 시각화"}</h2>
                <div class="analyzer-container">
                    <PitchAnalyzer show_links={Some(false)} />
                </div>
                <div class="description">
                    <h3>{"음높이 시각화 도구 활용법"}</h3>
                    <p>{"이 도구는 실시간으로 입력된 소리의 주파수를 그래프로 시각화합니다."}</p>
                    <p>{"마이크를 활성화하고 노래나 악기 소리를 입력하면 시간에 따른 음높이 변화를 확인할 수 있습니다."}</p>
                    <p>{"음악 연습, 발성 훈련, 음악 분석 등 다양한 용도로 활용해보세요."}</p>
                    <p>{"차트를 클릭하고 드래그하여 특정 부분을 확대할 수 있으며, 더블클릭하면 원래 보기로 돌아갑니다."}</p>
                </div>
            </div>
        </div>
    }
}

#[function_component(AmplitudeVisualizerDetail)]
pub fn amplitude_visualizer_detail() -> Html {
    // 진폭 시각화 컴포넌트용 상태 (시연 데이터)
    let dummy_history = use_state(|| {
        let mut history = VecDeque::new();
        for i in 0..100 {
            let time = i as f64 * 0.1;
            let amplitude = (time * 0.5).sin().abs() as f32 * 0.5;
            history.push_back((time, amplitude));
        }
        history
    });

    html! {
        <div class="detail-page">
            <div class="back-link">
                <Link<Route> to={Route::Home}>{"🏠 메인화면으로 돌아가기"}</Link<Route>>
            </div>
            <div class="content full-width">
                <h2>{"진폭 시각화"}</h2>
                <div class="analyzer-container">
                    <PitchAnalyzer show_links={Some(false)} />
                </div>
                <div class="description">
                    <h3>{"진폭 시각화 도구 활용법"}</h3>
                    <p>{"이 도구는 마이크 입력의 진폭을 실시간으로 그래프로 시각화합니다."}</p>
                    <p>{"마이크를 활성화하고 소리를 입력하면 시간에 따른 소리의 크기 변화를 확인할 수 있습니다."}</p>
                    <p>{"볼륨 레벨 모니터링, 소음 분석, 음성 패턴 분석 등에 활용할 수 있습니다."}</p>
                    <p>{"차트 설정을 조절하여 표시되는 시간 범위를 변경하거나 자동 스크롤을 켜고 끌 수 있습니다."}</p>
                </div>
            </div>
        </div>
    }
}

#[function_component(MetronomeDetail)]
pub fn metronome_detail() -> Html {
    html! {
        <div class="detail-page">
            <div class="back-link">
                <Link<Route> to={Route::Home}>{"🏠 메인화면으로 돌아가기"}</Link<Route>>
            </div>
            <div class="content full-width">
                <h2>{"메트로놈"}</h2>
                <div class="analyzer-container">
                    <PitchAnalyzer show_links={Some(false)} />
                </div>
                <div class="description">
                    <h3>{"메트로놈 사용법"}</h3>
                    <p>{"메트로놈은 음악의 박자를 측정하는 도구입니다."}</p>
                    <p>{"마이크를 활성화하고 음악을 재생하면 박자를 확인할 수 있습니다."}</p>
                    <p>{"음악 연습, 발성 훈련, 음악 분석 등에 활용해보세요."}</p>
                </div>
            </div>
        </div>
    }
}

#[function_component(ScaleGeneratorDetail)]
pub fn scale_generator_detail() -> Html {
    html! {
        <div class="detail-page">
            <div class="back-link">
                <Link<Route> to={Route::Home}>{"🏠 메인화면으로 돌아가기"}</Link<Route>>
            </div>
            <div class="content full-width">
                <h2>{"스케일 생성기"}</h2>
                <div class="analyzer-container">
                    <crate::tools::scale_generator::ScaleGenerator />
                </div>
                <div class="description">
                    <h3>{"스케일 생성기 사용법"}</h3>
                    <p>{"이 스케일 생성기를 사용하여 다양한 음악 스케일을 생성하고 연습할 수 있습니다."}</p>
                    <p>{"시작 근음과 종료 근음을 설정하고, 스케일 내 음 간격과 음정을 지정하여 연습에 활용하세요."}</p>
                    <p>{"상행/하행 옵션을 통해 다양한 방식의 스케일 연습이 가능합니다."}</p>
                </div>
            </div>
        </div>
    }
}

// 피아노 키보드 상세 페이지 컴포넌트
#[function_component(PianoKeyboardDetail)]
pub fn piano_keyboard_detail() -> Html {
    html! {
        <div class="detail-page">
            <div class="back-link">
                <Link<Route> to={Route::Home}>{"🏠 메인화면으로 돌아가기"}</Link<Route>>
            </div>
            <div class="content full-width">
                <h2>{"피아노 키보드"}</h2>
                <div class="analyzer-container">
                    <crate::tools::piano::Piano />
                </div>
                <div class="description">
                    <h3>{"피아노 키보드 사용법"}</h3>
                    <p>{"이 피아노 키보드를 사용하여 다양한 음을 연주하고 음악 연습을 할 수 있습니다."}</p>
                    <p>{"각 건반을 클릭하면 해당 음이 재생되며, 여러 건반을 동시에 눌러 화음을 연주할 수 있습니다."}</p>
                    <p>{"서스테인 버튼을 활성화하면 건반에서 손을 떼도 소리가 계속 유지됩니다."}</p>
                    <p>{"옥타브 조절 버튼을 사용하여 다양한 음역대의 건반을 표시할 수 있습니다."}</p>
                </div>
            </div>
        </div>
    }
}

#[function_component(NotFound)]
pub fn not_found() -> Html {
    html! {
        <div>
            <Link<Route> to={Route::Home}>{"🏠 Back to Home"}</Link<Route>>
        </div>
    }
}

// 피치 분석 컨트롤 컴포넌트
#[function_component(PitchControls)]
pub fn pitch_controls() -> Html {
    let sensitivity = use_state(|| 0.01f32);
    let show_sensitivity = use_state(|| false);
    let mic_active = use_state(|| false);
    let monitor_active = use_state(|| false);
    let is_playing = use_state(|| false);
    let has_recorded = use_state(|| true);
    let speaker_gain = use_state(|| 0.02f32);
    let show_download_format = use_state(|| false); // 다운로드 포맷 드롭다운 표시 상태
    let selected_format = use_state(|| "webm".to_string()); // 선택된 다운로드 포맷
    
    // 버튼 활성화/비활성화 상태 추가 - 로그를 통해 디버깅
    let buttons_disabled = use_state(|| false);
    
    // 재생 정보 상태 추가
    let current_time = use_state(|| 0.0f64);        // 현재 재생 시간
    let duration = use_state(|| 0.0f64);            // 총 녹음 시간
    let progress = use_state(|| 0.0f64);            // 진행률 (0~1)
    let is_seeking = use_state(|| false);           // 시크 중인지 여부

    // 재생 완료 이벤트 리스너 추가
    {
        let is_playing = is_playing.clone();
        let mic_active = mic_active.clone();
        
        use_effect(move || {
            let window = web_sys::window().expect("window를 찾을 수 없습니다");
            let document = window.document().expect("document를 찾을 수 없습니다");
            
            let is_playing_clone = is_playing.clone();
            let mic_active_clone = mic_active.clone();
            
            let callback = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                // 재생이 끝나면 재생 상태 변경 및 마이크 활성화
                is_playing_clone.set(false);
                mic_active_clone.set(false);
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "playbackEnded", 
                callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 메모리 누수 방지를 위해 클로저 유지
            callback.forget();
            
            // 클린업 함수
            || {}
        });
    }
    
    // 컨트롤 상태 초기화 이벤트 리스너 추가
    {
        let mic_active = mic_active.clone();
        let monitor_active = monitor_active.clone();
        let is_playing = is_playing.clone();
        let has_recorded = has_recorded.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        let progress = progress.clone();
        let is_seeking = is_seeking.clone();
        
        use_effect(move || {
            let window = web_sys::window().expect("window를 찾을 수 없습니다");
            let document = window.document().expect("document를 찾을 수 없습니다");
            
            let callback = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                // 컨트롤 상태 초기화 (PitchAnalyzer가 초기화될 때 함께 초기화)
                mic_active.set(false);
                monitor_active.set(false);
                is_playing.set(false);
                has_recorded.set(false);
                current_time.set(0.0);
                duration.set(0.0);
                progress.set(0.0);
                is_seeking.set(false);
                
                web_sys::console::log_1(&"[PitchControls] 컨트롤 상태가 초기화되었습니다".into());
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "resetPitchAnalyzer", 
                callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 메모리 누수 방지를 위해 클로저 유지
            callback.forget();
            
            // 클린업 함수
            || {}
        });
    }
    
    // 버튼 비활성화 이벤트 처리 - 기본 use_effect로 변경
    {
        let buttons_disabled = buttons_disabled.clone();
    
        use_effect(move || {
            let window = web_sys::window().expect("window를 찾을 수 없습니다");
            let document = window.document().expect("document를 찾을 수 없습니다");
            
            let callback = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                // 컨트롤 상태 초기화 (PitchAnalyzer가 초기화될 때 함께 초기화)
                buttons_disabled.set(true);
                
                web_sys::console::log_1(&"[PitchControls] 컨트롤 버튼이 비활성화되었습니다 (이벤트 핸들러)".into());
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "disableControlButtons", 
                callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 메모리 누수 방지를 위해 클로저 유지
            callback.forget();
            
            // 클린업 함수
            || {}
        });
    }

    {
        let buttons_disabled = buttons_disabled.clone();

        use_effect(move || {
            let window = web_sys::window().expect("window를 찾을 수 없습니다");
            let document = window.document().expect("document를 찾을 수 없습니다");
            
            let callback = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                // 컨트롤 상태 초기화 (PitchAnalyzer가 초기화될 때 함께 초기화)
                buttons_disabled.set(false);
                
                web_sys::console::log_1(&"[PitchControls] 컨트롤 버튼이 활성화되었습니다 (이벤트 핸들러)".into());
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "enableControlButtons", 
                callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 메모리 누수 방지를 위해 클로저 유지
            callback.forget();
            
            // 클린업 함수
            || {}
        });
    }

    let on_sensitivity_change = {
        let sensitivity = sensitivity.clone();
        Callback::from(move |e: web_sys::Event| {
            let input = e
                .target()
                .unwrap()
                .dyn_into::<web_sys::HtmlInputElement>()
                .unwrap();
            let value = input.value().parse::<f32>().unwrap_or(0.01);
            sensitivity.set(value);

            // 감도 변경 이벤트 발생
            let event = CustomEvent::new_with_event_init_dict(
                "updateSensitivity",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_f64(value as f64)),
            )
            .unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .dispatch_event(&event)
                .unwrap();
        })
    };

    let on_sensitivity_input = {
        let sensitivity = sensitivity.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            let input = e
                .target()
                .unwrap()
                .dyn_into::<web_sys::HtmlInputElement>()
                .unwrap();
            let value = input.value().parse::<f32>().unwrap_or(0.01);
            sensitivity.set(value);

            // 감도 변경 이벤트 발생
            let event = CustomEvent::new_with_event_init_dict(
                "updateSensitivity",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_f64(value as f64)),
            )
            .unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .dispatch_event(&event)
                .unwrap();
        })
    };

    let toggle_sensitivity = {
        let show_sensitivity = show_sensitivity.clone();
        Callback::from(move |_| {
            show_sensitivity.set(!*show_sensitivity);
        })
    };

    let toggle_audio = {
        let mic_active = mic_active.clone();
        let is_playing = is_playing.clone();
        let has_recorded = has_recorded.clone();
        Callback::from(move |e: web_sys::MouseEvent| {
            if *is_playing {
                return;
            }
            
            // 클릭 이벤트는 항상 상태를 반전
            let new_state = !*mic_active;
            mic_active.set(new_state);
            
            if new_state {
                has_recorded.set(true);
            }

            // 토글 이벤트 발생
            let event = CustomEvent::new_with_event_init_dict(
                "toggleAudio",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_bool(new_state)),
            )
            .unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .dispatch_event(&event)
                .unwrap();
        })
    };

    let toggle_monitor = {
        let monitor_active = monitor_active.clone();
        let mic_active = mic_active.clone();
        Callback::from(move |_| {
            // 마이크 비활성 상태에서는 모니터링 활성화 불가
            if !*mic_active {
                return;
            }

            // 모니터링 상태 토글
            let new_state = !*monitor_active;
            monitor_active.set(new_state);

            // 모니터링 토글 이벤트 발생
            let event = CustomEvent::new_with_event_init_dict(
                "toggleMonitor",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_bool(new_state)),
            )
            .unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .dispatch_event(&event)
                .unwrap();
        })
    };
    
    // 재생/일시정지 토글 콜백 추가
    let toggle_playback = {
        let is_playing = is_playing.clone();
        let mic_active = mic_active.clone();
        let has_recorded = has_recorded.clone();
        Callback::from(move |_| {
            if *mic_active {
                return;
            }
            
            let new_state = !*is_playing;
            is_playing.set(new_state);
            
            if !new_state {
                mic_active.set(false);
            }
            
            let event = CustomEvent::new_with_event_init_dict(
                "togglePlayback",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_bool(new_state)),
            )
            .unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .dispatch_event(&event)
                .unwrap();
        })
    };

    // 스피커 게인 슬라이더
    let on_speaker_gain_change = {
        let speaker_gain = speaker_gain.clone();
        Callback::from(move |e: web_sys::Event| {
            let input = e.target().unwrap().dyn_into::<web_sys::HtmlInputElement>().unwrap();
            let value = input.value().parse::<f32>().unwrap_or(0.02);
            speaker_gain.set(value);

            // 스피커 게인 변경 이벤트 발생
            let event = CustomEvent::new_with_event_init_dict(
                "updateSpeakerVolume",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_f64(value as f64)),
            ).unwrap();
            web_sys::window().unwrap().document().unwrap().dispatch_event(&event).unwrap();
        })
    };

    // 게이지 바 이벤트 핸들러 - change 이벤트
    let on_progress_change = {
        let progress = progress.clone();
        let is_seeking = is_seeking.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        Callback::from(move |e: web_sys::Event| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    // input의 value 값 읽기
                    let value = input.value().parse::<f64>().unwrap_or(0.0);
                    
                    // 1. 먼저 React 상태 업데이트
                    progress.set(value);
                    
                    // 2. 시간 값도 업데이트
                    if *duration > 0.0 {
                        let seek_time = value * *duration;
                        current_time.set(seek_time);
                    }
                    
                    // 3. Seek 이벤트 발생 (전역 이벤트)
                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    
                    let custom_event = CustomEvent::new_with_event_init_dict(
                        "seekPlayback",
                        CustomEventInit::new()
                            .bubbles(true)
                            .detail(&JsValue::from_f64(value)),
                    ).unwrap();
                    
                    // 4. 이벤트 발생 (main.rs에서 SeekPlayback 메시지 처리)
                    let _ = document.dispatch_event(&custom_event);
                    
                    // 5. 약간의 지연 후 강제로 DOM 업데이트 (closure 사용)
                    let input_clone = input.clone();
                    let value_clone = value;
                    
                    // setTimeout을 사용하여 비동기로 DOM 강제 업데이트
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        &Closure::once_into_js(move || {
                            input_clone.set_value(&value_clone.to_string());
                        }).as_ref().unchecked_ref(),
                        5, // 5ms 지연
                    );
                    
                    // 시크 종료 상태 설정
                    is_seeking.set(false);
                }
            }
        })
    };
    
    // 게이지 바 input 이벤트 핸들러 추가 (드래그 중 실시간 업데이트)
    let on_progress_input = {
        let progress = progress.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    // input의 value 값 읽기
                    let value = input.value().parse::<f64>().unwrap_or(0.0);
                    
                    // 1. 먼저 React 상태 업데이트
                    progress.set(value);
                    
                    // 2. 시간 값도 업데이트
                    if *duration > 0.0 {
                        let seek_time = value * *duration;
                        current_time.set(seek_time);
                    }
                    
                    // 3. Seek 이벤트 발생 (전역 이벤트)
                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    
                    let custom_event = CustomEvent::new_with_event_init_dict(
                        "seekPlayback",
                        CustomEventInit::new()
                            .bubbles(true)
                            .detail(&JsValue::from_f64(value)),
                    ).unwrap();
                    
                    // 4. 이벤트 발생 (main.rs에서 SeekPlayback 메시지 처리)
                    let _ = document.dispatch_event(&custom_event);
                }
            }
        })
    };
    
    // 시크 시작 및 종료 핸들러
    let on_seek_start = {
        let is_seeking = is_seeking.clone();
        let progress = progress.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        Callback::from(move |e: web_sys::MouseEvent| {
            is_seeking.set(true);
            
            // 마우스 이벤트 기록 (디버깅용)
            web_sys::console::log_1(&"마우스 드래그 시작".into());
            
            // 바로 클릭 위치에 게이지 위치 업데이트
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    // 요소의 위치와 크기 정보 가져오기
                    let rect = input.get_bounding_client_rect();
                    
                    // 요소 내에서의 상대적 위치 계산 (0~1 사이의 값으로 정규화)
                    let rel_x = (e.client_x() as f64 - rect.left()) / rect.width();
                    let value = rel_x.max(0.0).min(1.0); // 0~1 범위로 제한
                    
                    // 1. 첫 번째로 DOM에 직접 반영 (input의 value 속성)
                    input.set_value(&value.to_string());
                    
                    // 2. 상태 업데이트 (Yew 컴포넌트 상태)
                    progress.set(value);
                    
                    // 3. 시간 값도 업데이트
                    if *duration > 0.0 {
                        let seek_time = value * *duration;
                        current_time.set(seek_time);
                    }
                    
                    // 4. 비동기적으로 UI를 강제로 업데이트하는 이벤트 발생
                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    
                    // 입력 이벤트 발생
                    let input_event = web_sys::InputEvent::new("input").unwrap();
                    let _ = input.dispatch_event(&input_event);
                    
                    // change 이벤트 발생
                    let change_event = web_sys::Event::new("change").unwrap();
                    let _ = input.dispatch_event(&change_event);
                    
                    // 5. Seek 이벤트 발생 (전역 이벤트)
                    let custom_event = CustomEvent::new_with_event_init_dict(
                        "seekPlayback",
                        CustomEventInit::new()
                            .bubbles(true)
                            .detail(&JsValue::from_f64(value)),
                    ).unwrap();
                    
                    // 이벤트 발생 (main.rs에서 SeekPlayback 메시지 처리)
                    let _ = document.dispatch_event(&custom_event);
                    
                    // 6. 약간의 지연 후 강제로 DOM 업데이트 (closure 사용)
                    let input_clone = input.clone();
                    let value_clone = value;
                    
                    // setTimeout을 사용하여 비동기로 DOM 강제 업데이트
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        &Closure::once_into_js(move || {
                            input_clone.set_value(&value_clone.to_string());
                        }).as_ref().unchecked_ref(),
                        10, // 10ms 지연
                    );
                    
                    web_sys::console::log_1(&format!("클릭 위치: {:.2}, 게이지 값: {:.3}", rel_x, value).into());
                }
            }
        })
    };
    
    let on_seek_end = {
        let is_seeking = is_seeking.clone();
        Callback::from(move |e: web_sys::MouseEvent| {
            is_seeking.set(false);
            
            // 마우스 이벤트 기록 (디버깅용)
            web_sys::console::log_1(&"마우스 드래그 종료".into());
            
            // 드래그 종료 시 강제로 DOM 업데이트 이벤트 발생
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    // input 요소에 change 이벤트 발생
                    let change_event = web_sys::Event::new("change").unwrap();
                    let _ = input.dispatch_event(&change_event);
                }
            }
        })
    };
    
    // 터치 이벤트용 핸들러 (모바일용)
    let on_touch_start = {
        let is_seeking = is_seeking.clone();
        Callback::from(move |_: web_sys::TouchEvent| {
            is_seeking.set(true);
        })
    };
    
    let on_touch_move = {
        let progress = progress.clone();
        let is_seeking = is_seeking.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        Callback::from(move |e: web_sys::TouchEvent| {
            // 시크 중일 때만 처리
            if !*is_seeking {
                return;
            }
            
            // 기본 동작 방지
            e.prevent_default();
            
            // 터치 위치 정보 가져오기
            if e.touches().length() > 0 {
                let touch = e.touches().get(0).unwrap();
                
                if let Some(target) = e.target() {
                    if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                        // 요소의 위치와 크기 정보 가져오기
                        let rect = input.get_bounding_client_rect();
                        
                        // 요소 내에서의 상대적 위치 계산 (0~1 사이의 값으로 정규화)
                        let rel_x = (touch.client_x() as f64 - rect.left()) / rect.width();
                        let value = rel_x.max(0.0).min(1.0); // 0~1 범위로 제한
                        
                        // 1. 첫 번째로 DOM에 직접 반영 (input의 value 속성)
                        input.set_value(&value.to_string());
                        
                        // 2. 상태 업데이트 (Yew 컴포넌트 상태)
                        progress.set(value);
                        
                        // 3. 비동기적으로 UI를 강제로 업데이트하는 이벤트 발생
                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();
                        
                        // 입력 이벤트 발생
                        let input_event = web_sys::InputEvent::new("input").unwrap();
                        let _ = input.dispatch_event(&input_event);
                        
                        // change 이벤트 발생
                        let change_event = web_sys::Event::new("change").unwrap();
                        let _ = input.dispatch_event(&change_event);
                        
                        // 4. 시간 값도 업데이트
                        if *duration > 0.0 {
                            let seek_time = value * *duration;
                            current_time.set(seek_time);
                        }
                        
                        // 5. Seek 이벤트 발생 (전역 이벤트)
                        let custom_event = CustomEvent::new_with_event_init_dict(
                            "seekPlayback",
                            CustomEventInit::new()
                                .bubbles(true)
                                .detail(&JsValue::from_f64(value)),
                        ).unwrap();
                        
                        // 6. 이벤트 발생 (main.rs에서 SeekPlayback 메시지 처리)
                        let _ = document.dispatch_event(&custom_event);
                        
                        // 7. 약간의 지연 후 강제로 DOM 업데이트 (closure 사용)
                        let input_clone = input.clone();
                        let value_clone = value;
                        
                        // setTimeout을 사용하여 비동기로 DOM 강제 업데이트
                        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                            &Closure::once_into_js(move || {
                                input_clone.set_value(&value_clone.to_string());
                            }).as_ref().unchecked_ref(),
                            10, // 10ms 지연
                        );
                    }
                }
            }
        })
    };

    let on_touch_end = {
        let is_seeking = is_seeking.clone();
        Callback::from(move |_: web_sys::TouchEvent| {
            is_seeking.set(false);
        })
    };

    // 재생 시간 업데이트 이벤트 리스너 추가
    {
        let current_time = current_time.clone();
        let duration = duration.clone();
        let progress = progress.clone();
        let is_seeking = is_seeking.clone();
        let is_playing = is_playing.clone();
        let has_recorded = has_recorded.clone();
        let mic_active = mic_active.clone();
        
        use_effect(move || {
            let window = web_sys::window().expect("window를 찾을 수 없습니다");
            let document = window.document().expect("document를 찾을 수 없습니다");
            
            // 재생 시간 업데이트 이벤트 리스너
            let playback_time_callback = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                // 드래그 중에도 시간 정보는 업데이트 (단, 슬라이더 위치는 고정)
                let detail = e.detail();
                let data = js_sys::Object::from(detail);
                
                // 녹음 상태 확인 (녹음 중인지 여부)
                let is_recording = if let Ok(is_rec) = js_sys::Reflect::get(&data, &JsValue::from_str("isRecording")) {
                    if let Some(rec_state) = is_rec.as_bool() {
                        rec_state
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                if is_recording {
                    // 녹음 중일 때는 진행률을 0으로 고정하고, 현재 시간을 0으로 고정
                    progress.set(0.0);
                    current_time.set(0.0);
                    
                    // 녹음 중에는 마이크가 활성화되어 있어야 함
                    mic_active.set(true);
                    
                    // 전체 녹음 시간만 업데이트
                    if let Ok(total) = js_sys::Reflect::get(&data, &JsValue::from_str("duration")) {
                        if let Some(d) = total.as_f64() {
                            duration.set(d);
                        }
                    }
                } else {
                    // 일반 재생 모드에서는 정상적으로 시간 정보 업데이트
                    if let Ok(current) = js_sys::Reflect::get(&data, &JsValue::from_str("currentTime")) {
                        if let Some(time) = current.as_f64() {
                            current_time.set(time);
                        }
                    }
                    
                    if let Ok(total) = js_sys::Reflect::get(&data, &JsValue::from_str("duration")) {
                        if let Some(d) = total.as_f64() {
                            duration.set(d);
                            
                            // 시크 중이 아닐 때만 진행률 계산 및 업데이트
                            if !*is_seeking && d > 0.0 {
                                let prog = *current_time / d;
                                progress.set(prog);
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "playbackTimeUpdate", 
                playback_time_callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 재생 상태 업데이트 이벤트 리스너
            let state_callback = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                let detail = e.detail();
                
                if let Some(state) = detail.as_bool() {
                    is_playing.set(state);
                    
                    if state {
                        // 재생이 시작되면 has_recorded를 true로 설정
                        has_recorded.set(true);
                    }
                }
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "playbackStateChange", 
                state_callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 메모리 누수 방지를 위해 클로저 유지
            playback_time_callback.forget();
            state_callback.forget();
            
            // 클린업 함수
            || {}
        });
    }

    // 마이크 토글 이벤트 리스너
    {
        let mic_active = mic_active.clone();
        let is_playing = is_playing.clone();
        let has_recorded = has_recorded.clone();
        
        use_effect(move || {
            let window = web_sys::window().expect("window를 찾을 수 없습니다");
            let document = window.document().expect("document를 찾을 수 없습니다");
            
            // 서버에서 보내는 toggleAudio 이벤트 처리
            let callback = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                if *is_playing {
                    return;
                }
                
                // detail에 지정된 상태 가져오기
                let new_state = e.detail().as_bool().unwrap_or(!*mic_active);
                mic_active.set(new_state);
                
                if new_state {
                    has_recorded.set(true);
                }
                
                web_sys::console::log_1(&format!("서버에서 보낸 toggleAudio 이벤트 처리: new_state={}", new_state).into());
            }) as Box<dyn FnMut(_)>);
            
            document.add_event_listener_with_callback(
                "toggleAudio", 
                callback.as_ref().unchecked_ref()
            ).expect("이벤트 리스너 추가 실패");
            
            // 메모리 누수 방지를 위해 클로저 유지
            callback.forget();
            
            // 클린업 함수
            || {}
        });
    }

    // 시간 포맷 함수
    let format_time = |seconds: f64| -> String {
        let minutes = (seconds / 60.0).floor() as i32;
        let secs = (seconds % 60.0).floor() as i32;
        let ms = ((seconds % 1.0) * 100.0).round() as i32; // 밀리초 두 자리
        format!("{:02}:{:02}.{:02}", minutes, secs, ms)
    };

    // 마우스 이동 이벤트 핸들러 (드래그 중에 게이지 업데이트)
    let on_mouse_move = {
        let progress = progress.clone();
        let is_seeking = is_seeking.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        Callback::from(move |e: web_sys::MouseEvent| {
            // 시크 중일 때만 처리
            if !*is_seeking {
                return;
            }
            
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    // 요소의 위치와 크기 정보 가져오기
                    let rect = input.get_bounding_client_rect();
                    
                    // 요소 내에서의 상대적 위치 계산 (0~1 사이의 값으로 정규화)
                    let rel_x = (e.client_x() as f64 - rect.left()) / rect.width();
                    let value = rel_x.max(0.0).min(1.0); // 0~1 범위로 제한
                    
                    // 1. 첫 번째로 DOM에 직접 반영 (input의 value 속성)
                    input.set_value(&value.to_string());
                    
                    // 2. 상태 업데이트 (Yew 컴포넌트 상태)
                    progress.set(value);
                    
                    // 3. 시간 값도 업데이트
                    if *duration > 0.0 {
                        let seek_time = value * *duration;
                        current_time.set(seek_time);
                    }
                    
                    // 4. 비동기적으로 UI를 강제로 업데이트하는 이벤트 발생
                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    
                    // 입력 이벤트 발생
                    let input_event = web_sys::InputEvent::new("input").unwrap();
                    let _ = input.dispatch_event(&input_event);
                    
                    // 5. Seek 이벤트 발생 (전역 이벤트)
                    let custom_event = CustomEvent::new_with_event_init_dict(
                        "seekPlayback",
                        CustomEventInit::new()
                            .bubbles(true)
                            .detail(&JsValue::from_f64(value)),
                    ).unwrap();
                    
                    // 이벤트 발생 (main.rs에서 SeekPlayback 메시지 처리)
                    let _ = document.dispatch_event(&custom_event);
                    
                    // 6. 약간의 지연 후 강제로 DOM 업데이트 (closure 사용)
                    let input_clone = input.clone();
                    let value_clone = value;
                    
                    // setTimeout을 사용하여 비동기로 DOM 강제 업데이트
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        &Closure::once_into_js(move || {
                            input_clone.set_value(&value_clone.to_string());
                        }).as_ref().unchecked_ref(),
                        10, // 10ms 지연
                    );
                    
                    web_sys::console::log_1(&format!("마우스 이동: {:.2}, 게이지 값: {:.3}", rel_x, value).into());
                }
            }
        })
    };

    // 다운로드 포맷 토글 콜백
    let toggle_download_format = {
        let show_download_format = show_download_format.clone();
        Callback::from(move |_| {
            show_download_format.set(!*show_download_format);
        })
    };

    // 다운로드 포맷 선택 콜백
    let select_download_format = {
        let selected_format = selected_format.clone();
        Callback::from(move |format: String| {
            selected_format.set(format);
        })
    };

    // 다운로드 실행 콜백
    let execute_download = {
        let selected_format = selected_format.clone();
        let show_download_format = show_download_format.clone();
        Callback::from(move |_| {
            // 다운로드 이벤트 발생 (선택된 포맷 포함)
            let event = CustomEvent::new_with_event_init_dict(
                "downloadRecording",
                CustomEventInit::new()
                    .bubbles(true)
                    .detail(&JsValue::from_str(&selected_format)),
            ).unwrap();
            web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .dispatch_event(&event)
                .unwrap();
            
            // 드롭다운 닫기
            show_download_format.set(false);
            
            web_sys::console::log_1(&format!("다운로드 이벤트 발행됨 (포맷: {})", *selected_format).into());
        })
    };

    let buttons_disabled = buttons_disabled.clone();
    info!("buttons_disabled: {}", *buttons_disabled);

    html! {
        <div class="pitch-controls navbar-item">
            <div class="navbar-controls-buttons">
                <button
                    class={classes!("icon-button", if *mic_active { "mic-active" } else { "" })}
                    onclick={toggle_audio}
                    title={if *mic_active { "마이크 비활성화" } else { "마이크 활성화" }}
                    disabled={*is_playing || *buttons_disabled}
                >
                    { if *mic_active { "🔴" } else { "🎤" } }
                </button>
                <button
                    class={classes!("icon-button", if *monitor_active { "monitor-active" } else { "" })}
                    onclick={toggle_monitor}
                    title={if *monitor_active { "모니터링 비활성화" } else { "모니터링 활성화" }}
                    disabled={!*mic_active || *buttons_disabled}
                >
                    { if *monitor_active { "🔊" } else { "🔈" } }
                </button>
                
                <button
                    class={classes!("icon-button", if *is_playing { "play-active" } else { "" })}
                    onclick={toggle_playback}
                    title={if *is_playing { "일시정지" } else { "재생" }}
                    disabled={*mic_active || !*has_recorded || *buttons_disabled}
                >
                    { if *is_playing { "⏸️" } else { "▶️" } }
                </button>
                
                // 다운로드 버튼과 드롭다운 수정
                <div class="download-dropdown">
                    <button
                        class="icon-button download-button"
                        onclick={toggle_download_format}
                        title="녹음 파일 다운로드"
                        disabled={*mic_active || !*has_recorded || *buttons_disabled}
                    >
                        { "💾" }
                    </button>
                    {
                        if *show_download_format {
                            html! {
                                <div class="download-dropdown-content">
                                    <div class="format-option" onclick={let f = "webm".to_string(); select_download_format.clone().reform(move |_| f.clone())}>
                                        <span class={classes!("format-text", if *selected_format == "webm" { "selected" } else { "" })}>
                                            {"WebM"}
                                        </span>
                                    </div>
                                    <div class="format-option" onclick={let f = "mp3".to_string(); select_download_format.clone().reform(move |_| f.clone())}>
                                        <span class={classes!("format-text", if *selected_format == "mp3" { "selected" } else { "" })}>
                                            {"MP3"}
                                        </span>
                                    </div>
                                    <div class="format-option" onclick={let f = "wav".to_string(); select_download_format.clone().reform(move |_| f.clone())}>
                                        <span class={classes!("format-text", if *selected_format == "wav" { "selected" } else { "" })}>
                                            {"WAV"}
                                        </span>
                                    </div>
                                    <div class="format-option" onclick={let f = "ogg".to_string(); select_download_format.clone().reform(move |_| f.clone())}>
                                        <span class={classes!("format-text", if *selected_format == "ogg" { "selected" } else { "" })}>
                                            {"OGG"}
                                        </span>
                                    </div>
                                    <div class="format-option" onclick={let f = "m4a".to_string(); select_download_format.clone().reform(move |_| f.clone())}>
                                        <span class={classes!("format-text", if *selected_format == "m4a" { "selected" } else { "" })}>
                                            {"M4A"}
                                        </span>
                                    </div>
                                    <div class="download-separator"></div>
                                    <div class="format-option save-option" onclick={execute_download}>
                                        {"저장하기"}
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
                
                // 재생 게이지 바 추가
                {
                    html! {
                        <div class="playback-progress">
                            <span class="time-display current-time">{ format_time(*current_time) }</span>
                            <input 
                                type="range"
                                class="progress-bar"
                                min="0"
                                max="1"
                                step="0.001"
                                value={(*progress).to_string()}
                                onchange={on_progress_change}
                                oninput={on_progress_input}
                                onmousedown={on_seek_start}
                                onmouseup={on_seek_end}
                                onmousemove={on_mouse_move}
                                ontouchstart={on_touch_start}
                                ontouchmove={on_touch_move}
                                ontouchend={on_touch_end}
                                disabled={*mic_active || *buttons_disabled}
                                style="cursor: pointer;"
                            />
                            <span class="time-display duration">{ format_time(*duration) }</span>
                        </div>
                    }
                }
                
                <div class="sensitivity-dropdown">
                    <button class="icon-button" onclick={toggle_sensitivity} title="마이크 감도 조절">
                        { "🎚️" }
                    </button>
                    {
                        if *show_sensitivity {
                            html! {
                                <div class="sensitivity-dropdown-content">
                                    <div class="sensitivity-slider">
                                        <label for="speaker-gain">{"스피커 게인"}</label>
                                        <input
                                            type="range"
                                            id="speaker-gain"
                                            min="0.0"
                                            max="1.0"
                                            step="0.01"
                                            value={(*speaker_gain).to_string()}
                                            onchange={on_speaker_gain_change.clone()}
                                            disabled={*buttons_disabled}
                                        />
                                        <span>{ format!("{:.2}", *speaker_gain) }</span>
                                    </div>
                                    <div class="sensitivity-slider">
                                        <label for="sensitivity">{"감도"}</label>
                                        <input
                                            type="range"
                                            id="sensitivity"
                                            min="0.001"
                                            max="0.1"
                                            step="0.001"
                                            value={(*sensitivity).to_string()}
                                            onchange={on_sensitivity_change}
                                            oninput={on_sensitivity_input}
                                        />
                                        <span>{ format!("{:.3}", *sensitivity) }</span>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                // 상세 페이지 링크 버튼 추가
                <div class="icon-button">
                    <Link<Route> to={Route::PitchControls} classes={classes!("no-decoration")}>
                        { "🔗" }
                    </Link<Route>>
                </div>
            </div>
        </div>
    }
}

pub fn switch(routes: Route) -> Html {
    html! { <MainLayout /> }
}
