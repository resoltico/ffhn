use super::support::*;

#[test]
fn html_http_input_uses_the_final_response_url_as_its_htmlcut_base() {
    let target: TargetDocument = toml::from_str(
        "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"html-http\"\ndisplay_name = \"HTML HTTP\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"http\"\nsource_url = \"https://configured.example/request\"\n\n[fetch]\nengine = \"http\"\nuser_agent = \"ffhn-test\"\naccept = \"text/html\"\n\n[projection]\nkind = \"html_text\"\n\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = \"main\"\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = true\n",
    )
    .expect("HTML HTTP target");
    target.validate().expect("valid HTML HTTP target");
    let effective =
        url::Url::parse("https://redirected.example/content/page.html").expect("effective URL");

    let input = html_input(&target, "<main>7</main>", Some(&effective)).expect("HTML input");
    assert_eq!(
        input
            .input_base_url
            .as_ref()
            .expect("HTMLCut base URL")
            .as_fetch_str(),
        effective.as_str()
    );
}
