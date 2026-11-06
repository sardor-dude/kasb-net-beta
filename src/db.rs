use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    types::Json as SqlJson,
    SqlitePool,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub category: String,
    pub budget: i64,
    pub duration: String,
    pub skills: SqlJson<Vec<String>>,
    pub proposals_count: i64,
    pub client: String,
    pub kind: String,
    pub description: String,
    pub created_at: i64, // unix seconds; the frontend can turn this into "2 kun oldin"
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Service {
    pub title: String,
    pub price: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Freelancer {
    pub id: String,
    pub name: String,
    pub profession: String,
    pub bio: String,
    pub rating: f64,
    pub jobs_done: i64,
    pub verified_jobs: i64,
    pub from_price: i64,
    pub experience: String,
    pub has_avatar: bool,
    pub skills: SqlJson<Vec<String>>,
    pub services: SqlJson<Vec<Service>>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub name: String,
    pub contact: String,
    pub role: String, // "client" | "freelancer"
    pub created_at: i64,
}

pub async fn connect() -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::new()
        .filename("kasb.db")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
    migrate(&pool).await?;
    seed(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let stmts = [
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY, title TEXT NOT NULL, category TEXT NOT NULL,
            budget INTEGER NOT NULL, duration TEXT NOT NULL, skills TEXT NOT NULL,
            proposals_count INTEGER NOT NULL DEFAULT 0, client TEXT NOT NULL,
            kind TEXT NOT NULL, description TEXT NOT NULL, created_at INTEGER NOT NULL)",
        "CREATE TABLE IF NOT EXISTS freelancers (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, profession TEXT NOT NULL, bio TEXT NOT NULL,
            rating REAL NOT NULL, jobs_done INTEGER NOT NULL, verified_jobs INTEGER NOT NULL,
            from_price INTEGER NOT NULL, experience TEXT NOT NULL, has_avatar INTEGER NOT NULL,
            skills TEXT NOT NULL, services TEXT NOT NULL)",
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, contact TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL, created_at INTEGER NOT NULL)",
        "CREATE TABLE IF NOT EXISTS sessions (
            token TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id),
            created_at INTEGER NOT NULL)",
        "CREATE TABLE IF NOT EXISTS proposals (
            id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
            user_id TEXT NOT NULL REFERENCES users(id), price INTEGER NOT NULL,
            message TEXT NOT NULL, created_at INTEGER NOT NULL,
            UNIQUE (project_id, user_id))",
    ];
    for s in stmts {
        sqlx::query(s).execute(pool).await?;
    }
    Ok(())
}

fn sv(t: &str, p: i64) -> Service {
    Service { title: t.to_string(), price: p }
}

async fn seed(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects").fetch_one(pool).await?;
    if count > 0 {
        return Ok(());
    }
    let t = now();
    let (hour, day) = (3600_i64, 86_400_i64);

    let projects = [
        ("p1", "Kofexona uchun logotip va brend uslubi", "Dizayn", 4_500_000_i64, "10 kun",
         vec!["Logotip", "Figma", "Brendbuk"], 12_i64, "Sarvar Coffee", "Loyiha",
         "Toshkentdagi yangi kofexona uchun logotip, ranglar tizimi va qisqa brendbuk kerak. Ish 3 bosqichda: konsept, tanlangan variant, yakuniy fayllar.", 2 * day),
        ("p2", "Onlayn do’kon uchun UI/UX dizayn (mobil)", "UI/UX", 12_000_000, "3 hafta",
         vec!["Figma", "UX", "Mobil"], 8, "Bozorcha.uz", "Loyiha",
         "Mobil ilova uchun 24 ekranli prototip va dizayn tizimi. Mavjud tadqiqot materiallari beriladi.", day),
        ("p3", "Instagram va Telegram uchun SMM yuritish", "SMM", 3_000_000, "1 oy",
         vec!["SMM", "Kontent", "Telegram"], 21, "Nur Clinic", "Oylik",
         "Klinika uchun haftasiga 5 ta post, stories va Telegram kanal yuritish.", 5 * hour),
        ("p4", "Landing sahifa (React / Next.js)", "Dasturlash", 8_000_000, "2 hafta",
         vec!["Next.js", "TypeScript", "Tailwind"], 15, "EduStart", "Loyiha",
         "Tayyor Figma maketi asosida adaptiv landing sahifa va ariza formasi.", 3 * day),
        ("p5", "Taqdimot videosi montaji (90 soniya)", "Video", 2_200_000, "7 kun",
         vec!["Premiere", "Motion"], 9, "Orient Tex", "Loyiha",
         "Xom lavhalar asosida kompaniya taqdimot videosi, subtitr va musiqa bilan.", 4 * day),
        ("p6", "Veb-sayt matnlarini o’zbek tilida yozish", "Kopirayting", 1_500_000, "5 kun",
         vec!["Kopirayting", "O’zbek tili"], 17, "Lex Group", "Kichik vazifa",
         "Yuridik xizmatlar saytining 6 sahifasi uchun ishonchli, sodda matn.", 7 * day),
    ];
    for (id, title, cat, budget, dur, skills, props, client, kind, desc, ago) in projects {
        sqlx::query(
            "INSERT INTO projects (id,title,category,budget,duration,skills,proposals_count,client,kind,description,created_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id).bind(title).bind(cat).bind(budget).bind(dur)
        .bind(SqlJson(skills)).bind(props).bind(client).bind(kind).bind(desc)
        .bind(t - ago)
        .execute(pool)
        .await?;
    }

    let freelancers = [
        ("f1", "Dilnoza Karimova", "UI/UX dizayner",
         "Fintech va e-commerce mahsulotlari uchun interfeyslar loyihalayman.",
         4.9_f64, 34_i64, 28_i64, 1_500_000_i64, "5 yil", true,
         vec!["Figma", "UX", "Dizayn tizimi"],
         vec![sv("Mobil ilova dizayni", 6_000_000), sv("Landing dizayn", 2_500_000), sv("UX audit", 1_500_000)]),
        ("f2", "Jasur Rahimov", "Frontend dasturchi",
         "Next.js va TypeScript bilan tez, toza veb-ilovalar.",
         4.8, 22, 19, 2_500_000, "4 yil", true,
         vec!["Next.js", "TypeScript", "Tailwind"],
         vec![sv("Landing sahifa", 3_000_000), sv("Admin panel", 9_000_000)]),
        ("f3", "Madina Yusupova", "SMM mutaxassisi",
         "Mahalliy brendlar uchun kontent strategiya va yuritish.",
         4.7, 41, 36, 1_200_000, "3 yil", false,
         vec!["SMM", "Kontent", "Reels"],
         vec![sv("Oylik yuritish", 3_000_000), sv("Kontent reja", 1_200_000)]),
        ("f4", "Bekzod Aliyev", "Grafik dizayner",
         "Logotip, brend uslubi va qadoqlash dizayni.",
         4.6, 12, 6, 800_000, "1 yil", true,
         vec!["Logotip", "Illustrator", "Brendbuk"],
         vec![sv("Logotip", 800_000), sv("Brendbuk", 3_500_000)]),
    ];
    for (id, name, prof, bio, rating, done, ver, from, exp, av, skills, services) in freelancers {
        sqlx::query(
            "INSERT INTO freelancers (id,name,profession,bio,rating,jobs_done,verified_jobs,from_price,experience,has_avatar,skills,services)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id).bind(name).bind(prof).bind(bio).bind(rating).bind(done).bind(ver)
        .bind(from).bind(exp).bind(av).bind(SqlJson(skills)).bind(SqlJson(services))
        .execute(pool)
        .await?;
    }
    Ok(())
}
