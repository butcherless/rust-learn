//! Ejecuta el binario compilado. `--help`, los argumentos no válidos y un PostgreSQL cerrado no
//! necesitan nada; el recorrido completo usa el compose (ver `common`) y para el servicio con
//! SIGINT, como Ctrl+C, para que termine bien.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_escrutinio-servicio");

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("binary runs")
}

#[test]
fn help_lists_the_options() {
    let output = run(&["--help"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    [
        "--kafka",
        "--topic",
        "--topic-dlq",
        "--grupo",
        "--postgres",
        "--puerto",
        "GET /snapshots",
    ]
    .iter()
    .for_each(|part| assert!(stdout.contains(part), "missing {part:?}\n{stdout}"));
}

#[test]
fn invalid_arguments_exit_with_2() {
    [
        vec!["--puerto", "x"],
        vec!["--puerto", "70000"],
        vec!["--otra"],
    ]
    .iter()
    .for_each(|args| assert_eq!(run(args).status.code(), Some(2), "{args:?}"));
}

#[test]
fn exits_with_1_when_postgres_is_not_there() {
    let output = run(&["--postgres", "postgres://nadie@127.0.0.1:1/nada"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: no se pudo conectar con PostgreSQL: "),
        "{stderr}"
    );
}

/// `GET path` en `port`, o `None` si todavía no responde.
fn get(port: u16, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
}

/// Espera hasta 60 s a que `check` dé `true`.
fn wait_until(what: &str, check: impl Fn() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(start.elapsed() < Duration::from_secs(60), "timeout: {what}");
        thread::sleep(Duration::from_millis(200));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn consumes_a_station_and_serves_it_until_ctrl_c() {
    if !common::enabled("consumes_a_station_and_serves_it_until_ctrl_c") {
        return;
    }
    let (topic, dlq, group) = (
        common::unique("mesas"),
        common::unique("dlq"),
        common::unique("grupo"),
    );
    common::topics(&[&topic, &dlq]).await;
    let url = common::schema("cli").await;
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port();
    let child = Command::new(BIN)
        .args([
            "--topic",
            &topic,
            "--topic-dlq",
            &dlq,
            "--grupo",
            &group,
            "--postgres",
            &url,
        ])
        .args(["--puerto", &port.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("service starts");
    wait_until("the API answers", || {
        get(port, "/snapshots").as_deref() == Some("[]")
    });

    common::produce(&topic, "51", &common::json(&common::station("51", "A"))).await;
    wait_until("the station is served", || {
        get(port, "/snapshots").is_some_and(|body| body.contains(r#""province":"51","stations":1"#))
    });

    let pid = child.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-INT", &pid])
            .status()
            .expect("kill")
            .success()
    );
    let output = child.wait_with_output().expect("service ends");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with(&format!("Consumiendo {topic} de localhost:9092")),
        "{stdout}"
    );
    common::drop_topics(&[&topic, &dlq]).await;
    common::drop_schema(&url).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn borrar_empties_the_topics_and_the_tables() {
    if !common::enabled("borrar_empties_the_topics_and_the_tables") {
        return;
    }
    let (topic, dlq) = (common::unique("mesas"), common::unique("dlq"));
    common::topics(&[&topic, &dlq]).await;
    common::produce(&topic, "51", b"{}").await;
    let url = common::schema("borrar").await;
    let output = run(&[
        "borrar",
        "--topic",
        &topic,
        "--topic-dlq",
        &dlq,
        "--postgres",
        &url,
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Escrutinio borrado: 1 mensajes en Kafka y 0 mesas en PostgreSQL.\n"
    );
    common::drop_topics(&[&topic, &dlq]).await;
    common::drop_schema(&url).await;
}
