use super::super::*;

#[test]
fn public_markdown_links_and_repo_file_mentions_resolve() {
    let repo_root = repo_root();
    let tracked = git_tracked_relative_paths(&repo_root);

    for path in public_markdown_paths(&repo_root).expect("markdown paths") {
        let text = fs::read_to_string(&path).expect("read markdown");
        let path_display = path.display().to_string();

        for target in markdown_link_targets(&text) {
            if target.starts_with('#')
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }

            let resolved = resolve_repo_path(&repo_root, &path, &target);
            assert!(
                resolved.is_some(),
                "{path_display} links to missing local path `{target}`"
            );
            let resolved = resolved.expect("resolved markdown target");
            let relative = repo_relative_path(&repo_root, &resolved);
            if repo_root.join(&relative).is_file() {
                assert!(
                    tracked.contains(&relative),
                    "{path_display} links to repo file `{target}` that is not tracked by git"
                );
            }
        }

        for mention in repo_file_mentions(&text) {
            let resolved = resolve_repo_path(&repo_root, &path, &mention);
            assert!(
                resolved.is_some(),
                "{path_display} mentions missing repo file `{mention}`"
            );
            let resolved = resolved.expect("resolved repo file mention");
            let relative = repo_relative_path(&repo_root, &resolved);
            if repo_root.join(&relative).is_file() {
                assert!(
                    tracked.contains(&relative),
                    "{path_display} mentions repo file `{mention}` that is not tracked by git"
                );
            }
        }
    }
}
