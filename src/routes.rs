use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{types::Json as SqlJson, SqlitePool};
use uuid::Uuid;

use crate::{
    db::{now, Freelancer, Project, User},
    error::AppError,
    AppState,
};

const CATEGORIES: [&str; 6] = ["Dizayn", "UI/UX", "SMM", "Dasturlash", "Video", "Kopirayting"];

fn short_id(prefix: &str) -> String {
    format!("{prefix}{}", &Uuid::new_v4().simple().to_string()[..8])
}

/// Treat "" (what the frontend sends for "all") as "no filter".
fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

async fn authenticate(db: &SqlitePool, headers: &HeaderMap) -> Result<User, AppError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    sqlx::query_as::<_, User>(
        "SELECT u.id, u.name, u.contact, u.role, u.created_at
         FROM users u JOIN sessions s ON s.user_id = u.id WHERE s.token = ?",
    )
    .bind(token)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::Unauthorized)
}

// ---------- misc ----------

pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

pub async fn categories() -> Json<Value> {
    Json(json!(CATEGORIES))
}

// ---------- projects ----------

#[derive(Deserialize)]
pub struct ProjectQuery {
    q: Option<String>,
    category: Option<String>,
    budget_min: Option<i64>, // budget > budget_min
    budget_max: Option<i64>, // budget <= budget_max
    sort: Option<String>,    // n = newest (default), b = budget desc, p = fewest proposals
}

pub async fn list_projects(
    State(st): State<AppState>,
    Query(p): Query<ProjectQuery>,
) -> Result<Json<Vec<Project>>, AppError> {
    let like = non_empty(p.q).map(|q| format!("%{q}%"));
    let category = non_empty(p.category);

    // ORDER BY comes from a whitelist, never from user text
    let order = match p.sort.as_deref() {
        Some("b") => "budget DESC",
        Some("p") => "proposals_count ASC",
        _ => "created_at DESC",
    };
    let sql = format!(
        "SELECT * FROM projects
         WHERE (?1 IS NULL OR title LIKE ?1 OR description LIKE ?1 OR skills LIKE ?1)
           AND (?2 IS NULL OR category = ?2)
           AND (?3 IS NULL OR budget <= ?3)
           AND (?4 IS NULL OR budget > ?4)
         ORDER BY {order}"
    );
    let rows = sqlx::query_as::<_, Project>(&sql)
        .bind(like)
        .bind(category)
        .bind(p.budget_max)
        .bind(p.budget_min)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(rows))
}

pub async fn get_project(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Project>, AppError> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
pub struct NewProject {
    title: String,
    category: String,
    budget: i64,
    duration: String,
    #[serde(default)]
    skills: Vec<String>,
    description: String,
    kind: Option<String>,
}

pub async fn create_project(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(b): Json<NewProject>,
) -> Result<(StatusCode, Json<Project>), AppError> {
    let user = authenticate(&st.db, &headers).await?;
    if user.role != "client" {
        return Err(AppError::Forbidden("Loyihani faqat mijoz yarata oladi".into()));
    }
    if b.title.trim().chars().count() < 5 {
        return Err(AppError::BadRequest("Sarlavha juda qisqa".into()));
    }
    if !CATEGORIES.contains(&b.category.as_str()) {
        return Err(AppError::BadRequest("Noto’g’ri toifa".into()));
    }
    if b.budget <= 0 {
        return Err(AppError::BadRequest("Byudjet 0 dan katta bo’lishi kerak".into()));
    }
    if b.description.trim().is_empty() || b.duration.trim().is_empty() {
        return Err(AppError::BadRequest("Tavsif va muddatni kiriting".into()));
    }

    let id = short_id("p");
    sqlx::query(
        "INSERT INTO projects (id,title,category,budget,duration,skills,proposals_count,client,kind,description,created_at)
         VALUES (?,?,?,?,?,?,0,?,?,?,?)",
    )
    .bind(&id)
    .bind(b.title.trim())
    .bind(&b.category)
    .bind(b.budget)
    .bind(b.duration.trim())
    .bind(SqlJson(b.skills))
    .bind(&user.name)
    .bind(b.kind.unwrap_or_else(|| "Loyiha".into()))
    .bind(b.description.trim())
    .bind(now())
    .execute(&st.db)
    .await?;

    let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
        .bind(&id)
        .fetch_one(&st.db)
        .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

// ---------- proposals ----------

#[derive(Deserialize)]
pub struct NewProposal {
    price: i64,
    message: String,
}

pub async fn create_proposal(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(b): Json<NewProposal>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let user = authenticate(&st.db, &headers).await?;
    if user.role != "freelancer" {
        return Err(AppError::Forbidden("Taklifni faqat ijrochi yubora oladi".into()));
    }
    if b.price <= 0 {
        return Err(AppError::BadRequest("Narx 0 dan katta bo’lishi kerak".into()));
    }
    if b.message.trim().is_empty() {
        return Err(AppError::BadRequest("Iltimos, qisqa xat yozing".into()));
    }

    let mut tx = st.db.begin().await?;

    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE id = ?")
        .bind(&project_id)
        .fetch_one(&mut *tx)
        .await?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }

    let id = short_id("pr");
    let inserted = sqlx::query(
        "INSERT INTO proposals (id, project_id, user_id, price, message, created_at) VALUES (?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&project_id)
    .bind(&user.id)
    .bind(b.price)
    .bind(b.message.trim())
    .bind(now())
    .execute(&mut *tx)
    .await;

    match inserted {
        Ok(_) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AppError::Conflict("Siz bu loyihaga allaqachon taklif yuborgansiz".into()));
        }
        Err(e) => return Err(e.into()),
    }

    sqlx::query("UPDATE projects SET proposals_count = proposals_count + 1 WHERE id = ?")
        .bind(&project_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(json!({ "id": id, "project_id": project_id }))))
}

// ---------- freelancers ----------

#[derive(Deserialize)]
pub struct FreelancerQuery {
    q: Option<String>,
    profession: Option<String>,
    min_rating: Option<f64>,
}

pub async fn list_freelancers(
    State(st): State<AppState>,
    Query(p): Query<FreelancerQuery>,
) -> Result<Json<Vec<Freelancer>>, AppError> {
    let like = non_empty(p.q).map(|q| format!("%{q}%"));
    let profession = non_empty(p.profession);

    let rows = sqlx::query_as::<_, Freelancer>(
        "SELECT * FROM freelancers
         WHERE (?1 IS NULL OR name LIKE ?1 OR skills LIKE ?1 OR profession LIKE ?1)
           AND (?2 IS NULL OR profession = ?2)
           AND (?3 IS NULL OR rating >= ?3)
         ORDER BY rating DESC",
    )
    .bind(like)
    .bind(profession)
    .bind(p.min_rating)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

pub async fn get_freelancer(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Freelancer>, AppError> {
    sqlx::query_as::<_, Freelancer>("SELECT * FROM freelancers WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

// ---------- pricing calculator ----------

#[derive(Deserialize)]
pub struct CalcQuery {
    budget: i64,
    fee: Option<i64>, // freelancer fee percent, 5..=7 (default 6)
}

pub async fn pricing_calc(Query(q): Query<CalcQuery>) -> Result<Json<Value>, AppError> {
    let budget = q.budget.max(0);
    let fee = q.fee.unwrap_or(6);
    if !(5..=7).contains(&fee) {
        return Err(AppError::BadRequest("Komissiya 5 dan 7 gacha bo’lishi kerak".into()));
    }
    // integer math with round-half-up, same as Math.round in the frontend
    let client_fee = (budget * 3 + 50) / 100;
    let freelancer_fee = (budget * fee + 50) / 100;
    Ok(Json(json!({
        "budget": budget,
        "client_pays": budget + client_fee,
        "freelancer_gets": budget - freelancer_fee,
        "platform_revenue": client_fee + freelancer_fee,
    })))
}

// ---------- auth (demo level: no passwords yet) ----------

#[derive(Deserialize)]
pub struct Register {
    name: String,
    contact: String, // phone or Telegram @username
    role: String,    // "client" | "freelancer"
}

pub async fn register(
    State(st): State<AppState>,
    Json(b): Json<Register>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let name = b.name.trim();
    let contact = b.contact.trim();
    if name.chars().count() < 3 {
        return Err(AppError::BadRequest("Ism familiyani kiriting".into()));
    }
    if contact.chars().count() < 5 {
        return Err(AppError::BadRequest("Telefon yoki Telegram kiriting".into()));
    }
    if b.role != "client" && b.role != "freelancer" {
        return Err(AppError::BadRequest("Rolni tanlang".into()));
    }

    let id = short_id("u");
    let created = now();
    let res = sqlx::query("INSERT INTO users (id,name,contact,role,created_at) VALUES (?,?,?,?,?)")
        .bind(&id)
        .bind(name)
        .bind(contact)
        .bind(&b.role)
        .bind(created)
        .execute(&st.db)
        .await;
    match res {
        Ok(_) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AppError::Conflict("Bu kontakt bilan akkaunt mavjud".into()));
        }
        Err(e) => return Err(e.into()),
    }

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query("INSERT INTO sessions (token, user_id, created_at) VALUES (?,?,?)")
        .bind(&token)
        .bind(&id)
        .bind(created)
        .execute(&st.db)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "token": token,
            "user": { "id": id, "name": name, "contact": contact, "role": b.role }
        })),
    ))
}

pub async fn me(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<User>, AppError> {
    Ok(Json(authenticate(&st.db, &headers).await?))
}
