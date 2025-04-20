use std::collections::HashMap;
use yew::prelude::*;

/// 대시보드 아이템의 속성을 정의하는 구조체
#[derive(Properties, PartialEq, Clone)]
pub struct DashboardItem {
    pub id: String,
    pub component: Html,
    pub width: u32,  // 차지하는 격자 너비
    pub height: u32, // 차지하는 격자 높이
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
                    let item_style = format!(
                        "--item-width: {}; --item-height: {};",
                        item.width, item.height
                    );

                    html! {
                        <div
                            key={item.id.clone()}
                            class="dashboard-item"
                            style={item_style}
                        >
                            <div class="dashboard-item-content">
                                { item.component.clone() }
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
