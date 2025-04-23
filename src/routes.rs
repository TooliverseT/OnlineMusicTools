use wasm_bindgen::prelude::*;
use web_sys::{self, CustomEvent, CustomEventInit, Event};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::dashboard::{Dashboard, DashboardItem, DashboardLayout};
use crate::pitch_plot::PitchPlot;
use crate::PitchAnalyzer;

// 애플리케이션의 라우트 정의
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/pitch-controls")]
    PitchControls,
    #[at("/pitch-plot")]
    PitchPlot,
    #[not_found]
    #[at("/404")]
    NotFound,
}

// 네비게이션 바 컴포넌트
#[function_component(Navbar)]
pub fn navbar() -> Html {
    html! {
        <nav class="navbar">
            <div class="navbar-container">
                <Link<Route> classes={classes!("navbar-title")} to={Route::Home}>
                    {"MusicalMind"}
                </Link<Route>>
                <div class="navbar-controls">
                    <PitchControls />
                </div>
            </div>
        </nav>
    }
}

// 메인 레이아웃 컴포넌트
#[function_component(MainLayout)]
pub fn main_layout() -> Html {
    let location = use_location().unwrap();
    let route = Route::recognize(&location.path()).unwrap_or(Route::NotFound);

    // 현재 라우트에 따른 컨텐츠 선택
    let content = match route {
        Route::Home => html! { <PitchAnalyzer /> },
        Route::PitchControls => html! { <PitchControlsDetail /> },
        Route::PitchPlot => html! { <PitchPlotDetail /> },
        Route::NotFound => html! { <NotFound /> },
    };

    html! {
        <>
            <Navbar />
            <div class="app-container">
                { content }
            </div>
        </>
    }
}

// 상세 페이지 컴포넌트 - 피치 컨트롤
#[function_component(PitchControlsDetail)]
pub fn pitch_controls_detail() -> Html {
    html! {
        <div class="detail-page">
            <Link<Route> to={Route::Home}>{"🏠 Back to Home"}</Link<Route>>
            <div class="content">
                <PitchAnalyzer />
            </div>
        </div>
    }
}

// 상세 페이지 컴포넌트 - 피치 플롯
#[function_component(PitchPlotDetail)]
pub fn pitch_plot_detail() -> Html {
    // 빈 데이터로 PitchPlot 컴포넌트 렌더링
    // 실제 구현에서는 저장된 데이터를 불러오거나 API를 통해 데이터를 가져올 수 있음
    html! {
        <div class="detail-page">
            <Link<Route> to={Route::Home}>{"🏠 Back to Home"}</Link<Route>>
            <div class="content">
                <p>{"Detailed pitch analysis data and visualization."}</p>
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
    let show_links = use_state(|| true);
    let show_sensitivity = use_state(|| false);
    let mic_active = use_state(|| false);

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

    let toggle_links = {
        let show_links = show_links.clone();
        Callback::from(move |_| {
            let new_state = !*show_links;
            show_links.set(new_state);

            // 링크 토글 이벤트 발생
            let event = CustomEvent::new_with_event_init_dict(
                "toggleLinks",
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

    let toggle_sensitivity = {
        let show_sensitivity = show_sensitivity.clone();
        Callback::from(move |_| {
            show_sensitivity.set(!*show_sensitivity);
        })
    };

    let toggle_audio = {
        let mic_active = mic_active.clone();
        Callback::from(move |_| {
            // 마이크 상태 토글
            let new_state = !*mic_active;
            mic_active.set(new_state);

            // 마이크 토글 이벤트 발생
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

    html! {
        <div class="pitch-controls navbar-item">
            <div class="navbar-controls-buttons">
                <button
                    class={classes!("icon-button", if *mic_active { "mic-active" } else { "" })}
                    onclick={toggle_audio}
                    title={if *mic_active { "마이크 비활성화" } else { "마이크 활성화" }}
                >
                    { if *mic_active { "🔴" } else { "🎤" } }
                </button>
                <button class="icon-button" onclick={toggle_links} title={if *show_links { "링크 숨기기" } else { "링크 표시하기" }}>
                    { if *show_links { "🔗" } else { "🔓" } }
                </button>
                <div class="sensitivity-dropdown">
                    <button class="icon-button" onclick={toggle_sensitivity} title="마이크 감도 조절">
                        { "🎚️" }
                    </button>
                    {
                        if *show_sensitivity {
                            html! {
                                <div class="sensitivity-dropdown-content">
                                    <div class="sensitivity-slider">
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
                                        <span class="sensitivity-value">{ format!("{:.3}", *sensitivity) }</span>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </div>
    }
}

pub fn switch(routes: Route) -> Html {
    html! { <MainLayout /> }
}
