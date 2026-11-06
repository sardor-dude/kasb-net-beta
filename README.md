# kasb.net backend (Rust + Axum + SQLite)

## Run
1. Install Rust: https://rustup.rs
2. Put your frontend at `static/index.html`:
   `mkdir static && cp kasb_net___....html static/index.html`
3. `cargo run`  → http://localhost:3000 (frontend) and http://localhost:3000/api/...

The SQLite file `kasb.db` is created and seeded automatically on first start.

## API
| Method | Path | Notes |
|---|---|---|
| GET | /api/health | |
| GET | /api/categories | |
| GET | /api/projects | `q, category, budget_min, budget_max, sort=n\|b\|p` |
| GET | /api/projects/:id | |
| POST | /api/projects | client only, Bearer token |
| POST | /api/projects/:id/proposals | `{price, message}`, freelancer only, one per project |
| GET | /api/freelancers | `q, profession, min_rating` |
| GET | /api/freelancers/:id | |
| GET | /api/pricing/calc | `budget, fee=5..7` |
| POST | /api/auth/register | `{name, contact, role: "client"\|"freelancer"}` → `{token, user}` |
| GET | /api/me | Bearer token |

Send the token as `Authorization: Bearer <token>`.

## Try it
curl "localhost:3000/api/projects?category=Dizayn&sort=b"
curl -X POST localhost:3000/api/auth/register -H 'content-type: application/json' \
  -d '{"name":"Ali Valiyev","contact":"@ali_v","role":"freelancer"}'
