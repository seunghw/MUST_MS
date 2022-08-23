use actix_web::{
    get,
    middleware::Logger,
    post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};
use lettre::{transport::smtp::authentication::Credentials};
use lettre::{SmtpTransport};
use log::{debug, info};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha256::digest;
use sqlx::{
    postgres::{PgPoolOptions, PgQueryResult},
    PgPool,
};
use tokio;

const TOTAL_LEN: usize = 70;

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789";
const PASSWORD_LEN: usize = 8;

#[derive(Deserialize, Serialize)]
#[allow(non_snake_case)]
struct IDs {
    mailID: String,
    boxID: String,
}

fn create_random_string(len: usize) -> String {
    let mut rng = thread_rng();
    let rnd_str: String = (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    rnd_str
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct SQLRow {
    entry_id: i32,
    mail_id: String,
    box_id: String,
    schedule: Option<String>,
}

#[derive(Deserialize)]
struct CreateBoxBody {
    mails: Vec<String>,
}

#[post("/create")]
async fn create(mails: web::Json<CreateBoxBody>, pool: web::Data<PgPool>, base_url: web::Data<&str>) -> impl Responder {
    let box_id = create_random_string(PASSWORD_LEN);

    let mut conn = match (&pool).acquire().await {
        Ok(v) => v,
        Err(_) => return HttpResponse::InternalServerError().body("Cannot acquire connector"),
    };

    if mails.mails.len() == 0 {
        return HttpResponse::BadRequest().finish();
    }

    for mail in mails.mails.iter() {
        let res: Result<PgQueryResult, sqlx::Error> =
            sqlx::query(r#"INSERT INTO user_map (mail_id, box_id) VALUES ($1, $2);"#)
                .bind(digest(mail))
                .bind(&box_id)
                .execute(&mut conn)
                .await;
        if let Err(_) = res {
            return HttpResponse::InternalServerError().body("Fail to Insert");
        }
    }

    let creds = Credentials::new("hyunwook1202ha@gmail.com".to_string(), "keipbibjkitrfqjh".to_string());

    let mailer = SmtpTransport::relay("smtp.gmail.com")
    .unwrap()
    .credentials(creds)
    .build();


    for mail in mails.mails.iter() {
        let url = format!("{}?mailID={}&boxID={}", &(*base_url), digest(mail), &box_id);
        debug!("Link: {}", url);

        // let email = Message::builder()
        //     .from("MUST <hyunwook1202ha@dgist.ac.kr>".parse().unwrap())
        //     .to(mail.parse().unwrap())
        //     .subject("Invite Code")
        //     .body(url)
        //     .unwrap();
        
        // match mailer.send(&email) {
        //     Ok(_) => debug!("send Email to {}", &mail),
        //     Err(err) => debug!("Fail to Send Email to {} with error: {:?}", &mail, err),
        // }
    }

    let ids = IDs {
        mailID: digest(&mails.mails[0]),
        boxID: box_id,
    };

    if let Ok(v) = serde_json::to_string(&ids) {
        HttpResponse::Ok().body(v)
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct SubmitQuery {
    mailID: String,
    boxID: String,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct SubmitBody {
    mailID: String,
    boxID: String,
    idx: u32,
    pressed: bool
}

fn create_color_code(a: &Vec<u32>) -> Vec<String> {
    let mut ans = Vec::new();
    for i in a.iter() {
        let color = match i {
            0 => "#000000",
            1 => "#100000",
            2 => "#200000",
            3 => "#300000", 
            4 => "#400000",
            5 => "#500000", 
            6 => "#600000",
            7 => "#700000", 
            8 => "#800000",
            9 => "#900000", 
            10=> "#A00000",
            11=> "#B00000", 
            12=> "#C00000",
            13=> "#D00000", 
            14=> "#E00000",
            _=>  "#F00000", 
        }.to_string();
        ans.push(color);
    }
    ans
}

#[derive(Serialize, Deserialize)]
struct SubmitReturn {
    color: String
}

#[post("/submit")]
async fn submit(
    body: web::Json<SubmitBody>,
    pool: web::Data<PgPool>,
) -> impl Responder {

    let mail_id = body.mailID.clone();
    let box_id = body.boxID.clone();
    debug!("submit income: mail_id: {}, box_id {}", &mail_id, &box_id);

    let mut conn = match (&pool).acquire().await {
        Ok(v) => v,
        Err(_) => return HttpResponse::InternalServerError().body("Cannot acquire connector"),
    };

    let mut sch: Vec<u32> = match sqlx::query_as::<_, SQLRow>(
        r#"select entry_id, mail_id, box_id, schedule from user_map where box_id = $1 and mail_id = $2;"#,
    )
    .bind(&box_id)
    .bind(&mail_id)
    .fetch_one(&mut conn)
    .await
    {
        Ok(v) => serde_json::from_str::<Vec<u32>>(&v.schedule.unwrap_or_else(|| "[]".to_string())).unwrap(),
        Err(_) => return HttpResponse::InternalServerError().body("Fail to Select"),
    };

    let mut exist_flag = false;
    let mut tmp = Vec::new();
    for i in sch.into_iter() {
        if i != body.idx {
            tmp.push(i);
        } else {
            exist_flag = true;
        }
    }
    if !exist_flag {
        tmp.push(body.idx);
        tmp.sort();
    }
    sch = tmp;

    let res = sqlx::query(r#"Update user_map set schedule=$1 where mail_id=$2 and box_id=$3;"#)
        .bind(serde_json::to_string(&sch).unwrap())
        .bind(&mail_id)
        .bind(&box_id)
        .execute(&mut conn)
        .await;
    if let Err(err) = res {
        println!("{:?}", err);
        return HttpResponse::InternalServerError().body("Fail to Insert");
    }

    let mut ans: Vec<u32> = vec![0; TOTAL_LEN];
    for i in sch.iter() {
        ans[(*i) as usize] += 1;
    }

    // let body = create_color_code(&ans);
    // let body = serde_json::to_string(&body).unwrap();
    // println!("{}", &body);
    // HttpResponse::Ok().body(body)

    if exist_flag {
        HttpResponse::Ok().body("#000000")
    } else {
        HttpResponse::Ok().body("#FF0000")
    }
    
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct CheckResponse {
    doesSubmit: bool,
    numSubmit: u32,
    numUnSubmit: u32,
    mySchedule: String,
    allSubmit: bool,
    totalSchdule: String,
}

fn merge_vec(mut total_schedule: Vec<u32>, v: Vec<u32>) -> Vec<u32> {
    for i in v.iter() {
        total_schedule[(*i) as usize] += 1
    }
    return total_schedule;
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct CheckBody {
    mailID: String,
    boxID: String,
    idx: u32
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct Checkres {
    schedule: Vec<String>
}

#[get("/check")]
async fn check(query: web::Query<CheckBody>, pool: web::Data<PgPool>) -> impl Responder {
    let mail_id = query.mailID.clone();
    let box_id = query.boxID.clone();
    debug!("check income: mail_id: {}, box_id {}", &mail_id, &box_id);

    let mut conn = match (&pool).acquire().await {
        Ok(v) => v,
        Err(_) => return HttpResponse::InternalServerError().body("Cannot acquire connector"),
    };

    let rows: Vec<SQLRow> = match sqlx::query_as(
        r#"select entry_id, mail_id, box_id, schedule from user_map where box_id = $1;"#,
    )
    .bind(&box_id)
    .fetch_all(&mut conn)
    .await
    {
        Ok(v) => v,
        Err(_) => return HttpResponse::InternalServerError().body("Fail to Select"),
    };

    if rows.len() == 0 {
        return HttpResponse::BadRequest().finish();
    }

    let my_schdule = match rows.iter().rfind(|&x| x.mail_id == mail_id) {
        Some(v) => v.schedule.clone(),
        None => return HttpResponse::BadRequest().finish(),
    };

    let does_submit = my_schdule.is_some();

    let mut sub = 0;
    let mut unsub = 0;
    for item in rows.iter() {
        if item.schedule.is_some() {
            sub += 1;
        } else {
            unsub += 1;
        }
    }

    let mut total_schedule = vec![0; TOTAL_LEN];
    if unsub == 0 {
        for SQLRow { schedule: s, .. } in rows.iter() {
            if let Some(v) = s {
                let v: Vec<u32> = match serde_json::from_str(&v) {
                    Ok(v) => v,
                    Err(_) => {
                        return HttpResponse::InternalServerError().body("Fail to parse json")
                    }
                };

                total_schedule = merge_vec(total_schedule, v);
            } else {
                break;
            }
        }
    }

    let total_schedule = create_color_code(&total_schedule);

    let my_schedule = match my_schdule {
        Some(v) => match serde_json::from_str::<Vec<u32>>(&v) {
            Ok(v) => {
                let mut ans = vec![0; TOTAL_LEN];
                for i in v.iter() {
                    ans[(*i) as usize] += 1
                }
                ans
            },
            Err(_) => return HttpResponse::InternalServerError().body("Fail to parse json"),
        },
        None => vec![0; TOTAL_LEN],
    };

    let my_schedule = create_color_code(&my_schedule);

    let inner = match serde_json::to_string(&CheckResponse {
        doesSubmit: does_submit,
        numSubmit: sub,
        numUnSubmit: unsub,
        mySchedule: my_schedule[query.idx as usize].clone(),
        totalSchdule : total_schedule[query.idx as usize].clone(),
        allSubmit: unsub == 0,
    }) {
        Ok(v) => v,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    HttpResponse::Ok().body(inner)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(
            "<POSTGRESQL_URL>",
        )
        .await
        .unwrap();
    info!("DB Connected!");
    println!("Server Started!");
    
    let base_url = web::Data::new("49.161.13.6:8080/check");

    let pool_data = Data::new(pool);
    HttpServer::new(move || {
        App::new()
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(create)
            .service(check)
            .service(submit)
            .app_data(pool_data.clone())
            .app_data(base_url.clone())
            .wrap(Logger::default())
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
