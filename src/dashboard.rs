use crate::routes::Route;
use yew::prelude::*;
use yew_router::prelude::*;

/// 대시보드 아이템의 속성을 정의하는 구조체
#[derive(Properties, PartialEq, Clone)]
pub struct DashboardItem {
    pub id: String,
    pub component: Html,
    pub width: u32,           // 차지하는 격자 너비
    pub height: u32,          // 차지하는 격자 높이
    pub route: Option<Route>, // 상세 페이지 라우트
    #[prop_or(false)]
    pub show_link: bool, // 링크 버튼 표시 여부
    #[prop_or(1.0)]
    pub aspect_ratio: f32, // 가로 세로 비율 (너비/높이)
    #[prop_or(None)]
    pub custom_style: Option<String>, // 커스텀 CSS 스타일
}

/// 대시보드 레이아웃 속성을 정의하는 구조체
#[derive(Properties, PartialEq, Clone)]
pub struct DashboardLayout {
    pub items: Vec<DashboardItem>,
    pub columns: u32, // 총 격자 열 수
}

/// 대시보드 컴포넌트 속성
#[derive(Properties, PartialEq)]
pub struct DashboardProps {
    pub layout: DashboardLayout,
}

/// 대시보드 컴포넌트 정의
#[function_component(Dashboard)]
pub fn dashboard(props: &DashboardProps) -> Html {
    // 대시보드에 출력할 아이템들
    let items = &props.layout.items;
    let columns = props.layout.columns;

    // 스타일 CSS 변수 생성
    let dashboard_style = format!("--dashboard-columns: {};", columns);

    html! {
        <div class="dashboard" style={dashboard_style}>
            {
                items.iter().map(|item| {
                    // 기본 스타일에 커스텀 스타일 추가
                    let mut item_style = format!(
                        "--item-width: {}; --item-height: {}; --item-aspect-ratio: {};",
                        item.width, item.height, item.aspect_ratio
                    );
                    
                    // 커스텀 스타일이 있으면 추가
                    if let Some(custom) = &item.custom_style {
                        item_style.push_str(" ");
                        item_style.push_str(custom);
                    }

                    html! {
                        <div
                            key={item.id.clone()}
                            class="dashboard-item"
                            style={item_style}
                        >
                            <div class="dashboard-item-content" style="position: relative; width: 100%; height: 100%;">
                                { item.component.clone() }
                                {
                                    // 링크 표시 여부에 따라 링크 아이콘 추가
                                    if item.show_link && item.route.is_some() {
                                        html! {
                                            <div class="dashboard-item-link">
                                                <Link<Route> to={item.route.clone().unwrap()}>
                                                    { "🔗" }
                                                </Link<Route>>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        </div>
                    }
                }).collect::<Html>()
            }
        </div>
    }
}

/// 대시보드에 추가할 수 있는 간단한 카드 컴포넌트
#[derive(Properties, PartialEq)]
pub struct CardProps {
    pub title: String,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Card)]
pub fn card(props: &CardProps) -> Html {
    html! {
        <div class="dashboard-card">
            <div class="dashboard-card-header">
                <h3>{ &props.title }</h3>
            </div>
            <div class="dashboard-card-content">
                { for props.children.iter() }
            </div>
        </div>
    }
}
