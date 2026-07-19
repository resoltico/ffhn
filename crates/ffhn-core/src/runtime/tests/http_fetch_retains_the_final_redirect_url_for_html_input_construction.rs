use super::support::*;

#[test]
fn http_fetch_retains_the_final_redirect_url_for_html_input_construction() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        for response in [
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .into_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\n<main>7</main>".to_vec(),
        ] {
            let (mut stream, _) = listener.accept().expect("request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            stream.write_all(&response).expect("response");
        }
    });
    let headers = BTreeMap::new();
    let source = fetch_http_response(
        &url::Url::parse(&format!("http://{address}/start")).expect("URL"),
        1_000,
        1024,
        "ffhn-test",
        true,
        "text/html",
        &headers,
    )
    .expect("redirected response");
    worker.join().expect("server worker");

    assert_eq!(source.body, "<main>7</main>");
    assert_eq!(
        source
            .effective_http_url
            .as_ref()
            .expect("effective response URL")
            .as_str(),
        format!("http://{address}/final")
    );
}
