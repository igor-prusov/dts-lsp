use std::path::Path;

impl Backend {
    async fn mock_open(&self, uri: &str) {
        let file_data = read_to_string(uri).unwrap();

        let path = Path::new(uri).canonicalize().unwrap();
        let uri = Url::from_file_path(path).unwrap();
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(uri, "dts".to_owned(), 1, file_data),
        };

        self.did_open(params).await;
    }

    async fn mock_change(&self, uri: &str, file_data: String) {
        let path = Path::new(uri).canonicalize().unwrap();

        let uri = Url::from_file_path(path).unwrap();
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri, 2),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: file_data,
            }],
        };

        self.did_change(params).await;
    }

    async fn has_label(&self, path: &str, label: &str) -> usize {
        self.data.ld.find_label(&make_url(path), label).await.len()
    }

    async fn mock_rename_prepare(
        &self,
        uri: &str,
        pos: Position,
    ) -> Result<Option<PrepareRenameResponse>> {
        let path = Path::new(uri).canonicalize().unwrap();
        let uri = Url::from_file_path(path).unwrap();
        let params = TextDocumentPositionParams {
            position: pos,
            text_document: TextDocumentIdentifier::new(uri),
        };

        self.prepare_rename(params).await
    }

    async fn mock_rename(
        &self,
        uri: &str,
        pos: Position,
        new_name: &str,
    ) -> Result<Option<WorkspaceEdit>> {
        let path = Path::new(uri).canonicalize().unwrap();
        let uri = Url::from_file_path(path).unwrap();
        let params = RenameParams {
            new_name: new_name.to_string(),
            text_document_position: TextDocumentPositionParams {
                position: pos,
                text_document: TextDocumentIdentifier::new(uri),
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        self.rename(params).await
    }

    async fn mock_refrences(&self, uri: &str, pos: Position) -> Result<Option<Vec<Location>>> {
        let path = Path::new(uri).canonicalize().unwrap();
        let uri = Url::from_file_path(path).unwrap();
        let params = ReferenceParams {
            context: ReferenceContext {
                include_declaration: false,
            },
            text_document_position: TextDocumentPositionParams {
                position: pos,
                text_document: TextDocumentIdentifier::new(uri),
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        self.references(params).await
    }

    async fn verify_file(&self, uri: &Url, expected_file: &str) {
        let text = self.data.fd.get_text(uri).await.unwrap();
        let expected_text = read_to_string(expected_file).unwrap();
        assert_eq!(text, expected_text);
    }
    async fn test_edit(&self, uri: &Url, edit: TextEdit, expected_file: &str) {
        self.data.fd.apply_edits(uri, &vec![edit]).await;
        self.verify_file(uri, expected_file).await;
    }
}

fn make_url(path: &str) -> Url {
    let path = Path::new(path).canonicalize().unwrap();
    Url::from_file_path(path).unwrap()
}

fn make_range(begin: (u32, u32), end: (u32, u32)) -> Range {
    let begin = Position::new(begin.0, begin.1);
    let end = Position::new(end.0, end.1);
    Range::new(begin, end)
}

impl LabelsDepot {
    async fn new_expected(fd: &FileDepot, data: Vec<(&str, &str, Range)>) -> Self {
        let ld = LabelsDepot::new(fd);
        for x in data {
            ld.add_label(x.0, &make_url(x.1), x.2).await;
        }
        ld
    }
}

impl ReferencesDepot {
    async fn new_expected(fd: &FileDepot, data: Vec<(&str, &str, Range)>) -> Self {
        let rd = ReferencesDepot::new(fd);
        for x in data {
            rd.add_reference(x.0, &make_url(x.1), x.2).await;
        }
        rd
    }
}

#[derive(Debug, PartialEq)]
struct Changes(Result<Option<WorkspaceEdit>>);

impl Changes {
    // TODO: add new_expected
    fn new() -> Self {
        Self(Ok(Some(WorkspaceEdit::new(HashMap::new()))))
    }
    fn add_edit(&mut self, uri: &str, begin: (u32, u32), end: (u32, u32), new_text: &str) {
        let path = Path::new(uri).canonicalize().unwrap();
        let uri = Url::from_file_path(path).unwrap();

        let x = &mut self.0.as_mut().unwrap().as_mut().unwrap();
        let changes = x.changes.get_or_insert(HashMap::new());

        changes.entry(uri).or_default().push(TextEdit {
            range: Range::new(Position::new(begin.0, begin.1), Position::new(end.0, end.1)),
            new_text: new_text.to_string(),
        });
    }
}

use super::*;
#[tokio::test]
async fn functional() {
    Logger::set(&Logger::Print);
    {
        let be = Backend {
            data: Data::new(),
            process_neighbours: false,
        };
        let path = "tests/1/bad_file.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 1);
        assert_eq!(be.data.ld.size().await, 0);
        assert_eq!(be.data.rd.size().await, 0);
    }
    {
        let be = Backend::new();
        let path = "tests/1/good_file.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        assert_eq!(be.has_label(path, "a").await, 1);
        assert_eq!(be.has_label(path, "b").await, 1);
        assert_eq!(be.has_label(path, "label").await, 0);
    }
    {
        let be = Backend::new();
        let bad_path = "tests/1/bad_file.dts";
        let good_path = "tests/1/good_file.dts";

        be.mock_open(bad_path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        assert_eq!(be.has_label(good_path, "a").await, 1);
        assert_eq!(be.has_label(good_path, "b").await, 1);
        assert_eq!(be.has_label(good_path, "label").await, 0);
    }
    {
        /* Files without supported extension should be ignored */
        let be = Backend::new();
        let bad_ext_path = "tests/1/good_file.bad_ext";
        let good_path = "tests/1/good_file.dts";

        be.mock_open(bad_ext_path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        assert_eq!(be.has_label(good_path, "a").await, 1);
        assert_eq!(be.has_label(good_path, "b").await, 1);
        assert_eq!(be.has_label(good_path, "label").await, 0);
    }
    {
        let be = Backend::new();
        let path = "tests/2/b.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 1);

        assert_eq!(be.has_label(path, "node").await, 1);
    }
    {
        let be = Backend::new();
        let path = "tests/3/a.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 1);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        be.mock_change(
            path,
            "
                    / {
                        c: node_c{};
                    };
                    "
            .to_string(),
        )
        .await;
        assert_eq!(be.data.fd.size().await, 1);
        assert_eq!(be.data.ld.size().await, 1);
    }
    {
        /* File including non-existent DTSI file */
        let be = Backend::new();
        let path = "tests/4/a.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.fd.n_with_text().await, 1);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);
    }
    {
        let be = Backend::new();
        let path = "tests/1/good_file.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.fd.n_with_text().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        let res = be.mock_rename_prepare(path, Position::new(0, 0)).await;
        assert!(res.is_err());

        let res = be.mock_rename_prepare(path, Position::new(1, 0)).await;
        assert!(res.is_err());

        let res = be.mock_rename_prepare(path, Position::new(1, 2)).await;
        assert!(res.is_err());

        let res = be.mock_rename_prepare(path, Position::new(1, 1)).await;
        assert!(res.is_ok());

        let res = be.mock_rename(path, Position::new(0, 0), "changed").await;
        assert!(res.is_err());

        let res = be.mock_rename(path, Position::new(1, 0), "changed").await;
        assert!(res.is_err());

        let res = be.mock_rename(path, Position::new(1, 2), "changed").await;
        assert!(res.is_err());

        let mut expected = Changes::new();
        expected.add_edit(path, (1, 1), (1, 2), "changed");
        let res = be.mock_rename(path, Position::new(1, 1), "changed").await;
        assert_eq!(expected.0, res);
    }
    {
        // TODO: test rd and ld after rename
        // TODO: test single file, multiple references
        // TODO: probably b.dts should change as well
        let be = Backend::new();
        let path = "tests/2/a.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("node", "tests/2/a.dts", make_range((3, 1), (3, 5))),
                ("node", "tests/2/b.dts", make_range((2, 1), (2, 5))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![("node", "tests/2/common.dtsi", make_range((2, 10), (2, 14)))],
        )
        .await;
        assert!(be.data.rd == expected_rd);

        let res = be.mock_rename_prepare(path, Position::new(3, 1)).await;
        assert!(res.is_ok());

        let res = be.mock_rename(path, Position::new(3, 1), "changed").await;
        let mut expected = Changes::new();
        expected.add_edit("tests/2/a.dts", (3, 1), (3, 5), "changed");
        expected.add_edit("tests/2/common.dtsi", (2, 10), (2, 14), "changed");
        assert_eq!(expected.0, res);

        assert_eq!(be.data.fd.size().await, 3);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("changed", "tests/2/a.dts", make_range((3, 1), (3, 8))),
                ("node", "tests/2/b.dts", make_range((2, 1), (2, 5))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        #[rustfmt::skip]
        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![
            ("changed", "tests/2/common.dtsi", make_range((2, 10), (2, 17))),
            ],
        ).await;
        assert!(be.data.rd == expected_rd);
    }
    {
        let be = Backend::new();
        let path = "tests/2/common.dtsi";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("node", "tests/2/a.dts", make_range((3, 1), (3, 5))),
                ("node", "tests/2/b.dts", make_range((2, 1), (2, 5))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![("node", "tests/2/common.dtsi", make_range((2, 10), (2, 14)))],
        )
        .await;
        assert!(be.data.rd == expected_rd);

        let res = be.mock_rename_prepare(path, Position::new(2, 10)).await;
        assert!(res.is_ok());

        let res = be.mock_rename(path, Position::new(2, 10), "changed").await;
        let mut expected = Changes::new();
        expected.add_edit("tests/2/a.dts", (3, 1), (3, 5), "changed");
        expected.add_edit("tests/2/b.dts", (2, 1), (2, 5), "changed");
        expected.add_edit("tests/2/common.dtsi", (2, 10), (2, 14), "changed");
        assert_eq!(expected.0, res);

        assert_eq!(be.data.fd.size().await, 3);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 1);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("changed", "tests/2/a.dts", make_range((3, 1), (3, 8))),
                ("changed", "tests/2/b.dts", make_range((2, 1), (2, 8))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        #[rustfmt::skip]
        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![
            ("changed", "tests/2/common.dtsi", make_range((2, 10), (2, 17))),
            ],
        ).await;
        assert!(be.data.rd == expected_rd);
    }
    {
        let be = Backend::new();
        let path = "tests/5/before.dts";

        be.mock_open(path).await;
        let edits: Vec<TextEdit> = vec![
            TextEdit::new(make_range((3, 21), (3, 31)), "lbl".to_string()),
            TextEdit::new(make_range((1, 1), (1, 11)), "lbl".to_string()),
            TextEdit::new(
                make_range((3, 21), (3, 24)),
                "very_long_label_value".to_string(),
            ),
            TextEdit::new(
                make_range((1, 1), (1, 4)),
                "very_long_label_value".to_string(),
            ),
        ];

        for (i, edit) in edits.iter().enumerate() {
            let index = i + 1;
            let expected_file = format!("tests/5/after-{index}.dts");
            be.test_edit(&make_url(path), edit.clone(), &expected_file)
                .await;
        }
    }
    {
        let be = Backend::new();
        let path = "tests/5/before.dts";

        be.mock_open(path).await;

        let _ = be.mock_rename(path, Position::new(1, 1), "lbl").await;
        be.verify_file(&make_url(path), "tests/5/after-2.dts").await;

        let _ = be
            .mock_rename(path, Position::new(1, 1), "some_label")
            .await;
        be.verify_file(&make_url(path), "tests/5/before.dts").await;

        let _ = be
            .mock_rename(path, Position::new(1, 1), "very_long_label_value")
            .await;
        be.verify_file(&make_url(path), "tests/5/after-4.dts").await;
    }
    {
        // find references breaks after buffer is changed and restored
        let be = Backend::new();
        let path = "tests/2/a.dts";
        let uri = make_url(path);

        be.mock_open(path).await;
        be.data.fd.apply_edits(&uri, &vec![]).await;
        let res = be.mock_refrences(path, Position::new(3, 1)).await;
        assert_eq!(res.unwrap().unwrap().len(), 1);

        let old_file_contents = be.data.fd.get_text(&uri).await.unwrap();
        be.mock_change(path, "".to_string()).await;
        be.mock_change(path, old_file_contents).await;

        let res = be.mock_refrences(path, Position::new(3, 1)).await;
        info!("{}", be.data.fd.get_text(&make_url(path)).await.unwrap());
        assert_eq!(res.unwrap().unwrap().len(), 1);
    }
}
