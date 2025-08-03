use anyhow::{Result, anyhow};
use askama::Template;
use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fmt::Display;

#[derive(Template)]
#[template(path = "user_stats.html")]
struct UserStatsTemplate {
    player_name: String,
    game_history: Vec<GameMatch>,
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html_content) => Html(html_content).into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

#[derive(Debug)]
struct GameMatch {
    player_rank: u64,
    start_time: DateTime<Utc>,
    duration_minutes: u64,
    game_type: GameType,
    pt_change: i64,
    player_results: Vec<PlayerResult>,
}

#[derive(Debug)]
struct PlayerResult {
    name: String,
    final_score: i64,
}

#[derive(Debug)]
enum GameRule {
    ThreePlayer,
    FourPlayer,
}

impl GameRule {
    fn api_base_url(&self) -> &'static str {
        match self {
            GameRule::ThreePlayer => "https://5-data.amae-koromo.com/api/v2/pl3",
            GameRule::FourPlayer => "https://5-data.amae-koromo.com/api/v2/pl4",
        }
    }

    fn supported_mode_ids(&self) -> &'static str {
        match self {
            GameRule::ThreePlayer => "21,22,23,24,25,26",
            GameRule::FourPlayer => "8,9,11,12,15,16",
        }
    }
}

#[derive(Debug)]
enum GameCategory {
    Gold,
    GoldEast,
    Jade,
    JadeEast,
    Throne,
    ThroneEast,
}

#[derive(Debug)]
struct GameType {
    rule: GameRule,
    category: GameCategory,
}

impl Display for GameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rule_prefix = match self.rule {
            GameRule::ThreePlayer => "3P",
            GameRule::FourPlayer => "4P",
        };

        let category_name = match self.category {
            GameCategory::GoldEast => "Gold East",
            GameCategory::Gold => "Gold",
            GameCategory::JadeEast => "Jade East",
            GameCategory::Jade => "Jade",
            GameCategory::ThroneEast => "Throne East",
            GameCategory::Throne => "Throne",
        };

        write!(f, "{} {}", rule_prefix, category_name)
    }
}

impl From<u64> for GameType {
    fn from(mode_id: u64) -> Self {
        match mode_id {
            21 => GameType {
                rule: GameRule::ThreePlayer,
                category: GameCategory::GoldEast,
            },
            22 => GameType {
                rule: GameRule::ThreePlayer,
                category: GameCategory::Gold,
            },
            23 => GameType {
                rule: GameRule::ThreePlayer,
                category: GameCategory::JadeEast,
            },
            24 => GameType {
                rule: GameRule::ThreePlayer,
                category: GameCategory::Jade,
            },
            25 => GameType {
                rule: GameRule::ThreePlayer,
                category: GameCategory::ThroneEast,
            },
            26 => GameType {
                rule: GameRule::ThreePlayer,
                category: GameCategory::Throne,
            },
            8 => GameType {
                rule: GameRule::FourPlayer,
                category: GameCategory::GoldEast,
            },
            9 => GameType {
                rule: GameRule::FourPlayer,
                category: GameCategory::Gold,
            },
            11 => GameType {
                rule: GameRule::FourPlayer,
                category: GameCategory::JadeEast,
            },
            12 => GameType {
                rule: GameRule::FourPlayer,
                category: GameCategory::Jade,
            },
            15 => GameType {
                rule: GameRule::FourPlayer,
                category: GameCategory::ThroneEast,
            },
            16 => GameType {
                rule: GameRule::FourPlayer,
                category: GameCategory::Throne,
            },
            _ => unreachable!("Invalid mode ID: {}", mode_id),
        }
    }
}

async fn find_player_id_by_name(player_name: &str, rule: &GameRule) -> Result<u64> {
    let search_url = format!(
        "{}/search_player/{}?tag=all",
        rule.api_base_url(),
        player_name
    );
    let response = reqwest::get(search_url).await?.json::<Value>().await?;

    if response.is_array() && !response.as_array().unwrap().is_empty() {
        Ok(response[0]["id"].as_u64().expect("Valid player ID"))
    } else {
        Err(anyhow!("No player found with name: {}", player_name))
    }
}

async fn handle_3p_player_stats(
    Path(player_name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_player_stats_request(player_name, GameRule::ThreePlayer).await
}

async fn handle_4p_player_stats(
    Path(player_name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    handle_player_stats_request(player_name, GameRule::FourPlayer).await
}

async fn handle_player_stats_request(
    player_name: String,
    rule: GameRule,
) -> Result<impl IntoResponse, StatusCode> {
    let player_id = find_player_id_by_name(&player_name, &rule)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let game_history = fetch_complete_match_history(player_id, &rule)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let template = UserStatsTemplate {
        player_name,
        game_history,
    };

    Ok(HtmlTemplate(template))
}

async fn fetch_complete_match_history(player_id: u64, rule: &GameRule) -> Result<Vec<GameMatch>> {
    let mut current_timestamp = chrono::Utc::now().timestamp();
    let mut all_matches = Vec::new();

    loop {
        let api_url = format!(
            "{}/player_records/{}/{}/1262304000000?limit=500&mode={}&descending=true",
            rule.api_base_url(),
            player_id,
            current_timestamp,
            rule.supported_mode_ids()
        );

        let response = reqwest::get(api_url).await?.json::<Value>().await?;

        let batch_matches = parse_match_data(&response, player_id);

        if batch_matches.is_empty() {
            break;
        }

        // Update timestamp for next batch
        current_timestamp = batch_matches.last().unwrap().start_time.timestamp() - 1;
        all_matches.extend(batch_matches);

        // Stop if we received less than the limit (last page)
        if response.as_array().unwrap().len() < 500 {
            break;
        }
    }

    Ok(all_matches)
}

fn parse_match_data(api_response: &Value, target_player_id: u64) -> Vec<GameMatch> {
    api_response
        .as_array()
        .unwrap()
        .iter()
        .map(|match_data| {
            let mut player_data = match_data["players"]
                .as_array()
                .unwrap()
                .iter()
                .map(|player| {
                    let player_id = player["accountId"].as_u64().unwrap();
                    let player_name = player["nickname"].as_str().unwrap().to_string();
                    let final_score = player["score"].as_i64().unwrap();
                    let pt_change = player["gradingScore"].as_i64().unwrap();
                    (player_id, player_name, final_score, pt_change)
                })
                .collect::<Vec<_>>();

            // Sort by pt change (descending) to determine ranking
            player_data.sort_by(|a, b| b.3.cmp(&a.3));

            let player_rank = player_data
                .iter()
                .position(|player| player.0 == target_player_id)
                .map(|position| (position + 1) as u64)
                .unwrap();

            let raw_start_time = match_data["startTime"].as_u64().unwrap();
            let raw_end_time = match_data["endTime"].as_u64().unwrap();

            let start_time = DateTime::<Utc>::from_timestamp(raw_start_time as i64, 0).unwrap();

            let duration_minutes = (raw_end_time - raw_start_time) / 60;

            let game_type = GameType::from(match_data["modeId"].as_u64().unwrap());

            let pt_change = player_data
                .iter()
                .find(|player| player.0 == target_player_id)
                .map(|player| player.3)
                .unwrap();

            let player_results = player_data
                .into_iter()
                .map(|(_, name, final_score, _)| PlayerResult { name, final_score })
                .collect();

            GameMatch {
                player_rank,
                start_time,
                duration_minutes,
                game_type,
                pt_change,
                player_results,
            }
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/search/3p/{name}", get(handle_3p_player_stats))
        .route("/search/4p/{name}", get(handle_4p_player_stats));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
