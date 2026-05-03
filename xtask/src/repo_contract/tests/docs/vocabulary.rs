use super::super::*;

#[test]
fn target_config_docs_present_cross_platform_notification_examples() {
    let repo_root = repo_root();
    let targets_doc =
        fs::read_to_string(repo_root.join("docs/targets.md")).expect("read docs/targets.md");

    assert!(
        targets_doc.contains("program = \"/bin/sh\""),
        "docs/targets.md must keep one POSIX notification example"
    );
    assert!(
        targets_doc.contains("PowerShell append-only JSONL sink"),
        "docs/targets.md must label the Windows notification example clearly"
    );
    assert!(
        targets_doc.contains("pwsh.exe"),
        "docs/targets.md must show one PowerShell notification example for Windows readers"
    );
    assert!(
        targets_doc.contains("examples/file-target-with-notifications/README.md"),
        "docs/targets.md must route readers to the checked-in cross-platform notification example"
    );
}

#[test]
fn public_markdown_mentions_only_registered_operations_and_documents() {
    let repo_root = repo_root();
    let registered_operations = ffhn_core::cli_contract()
        .operations
        .iter()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    let registered_documents = ffhn_core::cli_contract()
        .documents
        .iter()
        .map(|document| document.id)
        .collect::<BTreeSet<_>>();

    for path in public_markdown_paths(&repo_root).expect("markdown paths") {
        let text = fs::read_to_string(&path).expect("read markdown");
        let path_display = path.display().to_string();
        assert_registered_operation_ids(
            &path_display,
            extract_cli_operation_ids(&text),
            &registered_operations,
        );
        assert_registered_document_ids(
            &path_display,
            extract_document_ids(&text),
            &registered_documents,
        );
    }
}

#[test]
fn user_facing_source_literals_mention_only_registered_operations_and_documents() {
    let repo_root = repo_root();
    let registered_operations = ffhn_core::cli_contract()
        .operations
        .iter()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    let registered_documents = ffhn_core::cli_contract()
        .documents
        .iter()
        .map(|document| document.id)
        .collect::<BTreeSet<_>>();

    assert_registered_operation_ids(
        "inline literal",
        extract_cli_operation_ids("`ffhn run --target demo`"),
        &registered_operations,
    );
    assert_registered_document_ids(
        "inline literal",
        extract_document_ids("ffhn.run_report"),
        &registered_documents,
    );

    for path in user_facing_source_paths(&repo_root).expect("source paths") {
        let text = fs::read_to_string(&path).expect("read source");
        for literal in string_literals(production_source_text(&text)) {
            let path_display = path.display().to_string();
            assert_registered_operation_ids(
                &path_display,
                extract_cli_operation_ids(&literal),
                &registered_operations,
            );
            assert_registered_document_ids(
                &path_display,
                extract_document_ids(&literal),
                &registered_documents,
            );
        }
    }
}
