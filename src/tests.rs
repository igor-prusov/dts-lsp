use std::path::Path;

struct TestContext(Backend);

fn make_ctx(path: &str) -> TestContext {
    let path = Path::new(path);
    // Go to test directory, each test directory emulates a workspace
    std::env::set_current_dir(path).unwrap();
    TestContext(Backend {
        data: Data::new(),
        process_neighbours: true,
        client: None,
    })
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Go back to root directory
        let path = Path::new("../../");
        std::env::set_current_dir(path).unwrap();
    }
}

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
        self.data.ld.find_label(&make_url(path), label).len()
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

    async fn mock_goto_definition(
        &self,
        uri: &str,
        pos: Position,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let path = Path::new(uri).canonicalize().unwrap();
        let uri = Url::from_file_path(path).unwrap();
        let params = GotoDefinitionParams {
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            text_document_position_params: TextDocumentPositionParams {
                position: pos,
                text_document: TextDocumentIdentifier::new(uri),
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        self.goto_definition(params).await
    }

    async fn verify_file(&self, uri: &Url, expected_file: &str) -> bool {
        let text = self.data.fd.get_text(uri).unwrap();
        let expected_text = read_to_string(expected_file).unwrap();
        let res = text == expected_text;

        if !res {
            error!("---\ngot:\n{}\nexpected:\n{}\n---\n", text, expected_text);
        }

        res
    }
    async fn test_edit(&self, uri: &Url, edit: TextEdit, expected_file: &str) {
        self.data.fd.apply_edits(uri, &vec![edit]);
        let res = self.verify_file(uri, expected_file).await;
        assert!(res);
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
            ld.add_label(x.0, &make_url(x.1), x.2);
        }
        ld
    }
}

impl ReferencesDepot {
    async fn new_expected(fd: &FileDepot, data: Vec<(&str, &str, Range)>) -> Self {
        let rd = ReferencesDepot::new(fd);
        for x in data {
            rd.add_reference(x.0, &make_url(x.1), x.2);
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
    Logger::set(Logger::Print);
    {
        info!("TEST 0");
        let be = Backend {
            data: Data::new(),
            process_neighbours: false,
            client: None,
        };
        let path = "tests/1/bad_file.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 1);
        assert_eq!(be.data.ld.size().await, 0);
        assert_eq!(be.data.rd.size().await, 0);
    }
    {
        info!("TEST 1");
        let be = &make_ctx("tests/1/").0;
        let path = "good_file.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        assert_eq!(be.has_label(path, "a").await, 1);
        assert_eq!(be.has_label(path, "b").await, 1);
        assert_eq!(be.has_label(path, "label").await, 0);
    }
    {
        info!("TEST 2");
        let be = &make_ctx("tests/1/").0;
        let bad_path = "bad_file.dts";
        let good_path = "good_file.dts";

        be.mock_open(bad_path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        assert_eq!(be.has_label(good_path, "a").await, 1);
        assert_eq!(be.has_label(good_path, "b").await, 1);
        assert_eq!(be.has_label(good_path, "label").await, 0);
    }
    {
        info!("TEST 3");
        /* Files without supported extension should be ignored */
        let be = &make_ctx("tests/1/").0;
        let bad_ext_path = "good_file.bad_ext";
        let good_path = "good_file.dts";

        be.mock_open(bad_ext_path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);

        assert_eq!(be.has_label(good_path, "a").await, 1);
        assert_eq!(be.has_label(good_path, "b").await, 1);
        assert_eq!(be.has_label(good_path, "label").await, 0);
    }
    {
        info!("TEST 4");
        let be = &make_ctx("tests/2/").0;
        let path = "b.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 1);

        assert_eq!(be.has_label(path, "node").await, 1);
    }
    {
        info!("TEST 5");
        let be = &make_ctx("tests/3/").0;
        let path = "a.dts";

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
        info!("TEST 6");
        /* File including non-existent DTSI file */
        let be = &make_ctx("tests/4/").0;
        let path = "a.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 2);
        assert_eq!(be.data.fd.n_with_text().await, 1);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 0);
    }
    {
        info!("TEST 7");
        let be = &make_ctx("tests/1/").0;
        let path = "good_file.dts";

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
        info!("TEST 8");
        // TODO: test single file, multiple references
        // TODO: probably b.dts should change as well
        let be = &make_ctx("tests/2/").0;
        let path = "a.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("node", "a.dts", make_range((3, 1), (3, 5))),
                ("node", "b.dts", make_range((2, 1), (2, 5))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![("node", "common.dtsi", make_range((2, 10), (2, 14)))],
        )
        .await;
        assert!(be.data.rd == expected_rd);

        let res = be.mock_rename_prepare(path, Position::new(3, 1)).await;
        assert!(res.is_ok());

        let res = be.mock_rename(path, Position::new(3, 1), "changed").await;
        let mut expected = Changes::new();
        expected.add_edit("a.dts", (3, 1), (3, 5), "changed");
        expected.add_edit("common.dtsi", (2, 10), (2, 14), "changed");
        assert_eq!(expected.0, res);

        assert_eq!(be.data.fd.size().await, 3);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("changed", "a.dts", make_range((3, 1), (3, 8))),
                ("node", "b.dts", make_range((2, 1), (2, 5))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        #[rustfmt::skip]
        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![
            ("changed", "common.dtsi", make_range((2, 10), (2, 17))),
            ],
        ).await;
        assert!(be.data.rd == expected_rd);
    }
    {
        info!("TEST 9");
        let be = &make_ctx("tests/2/").0;
        let path = "common.dtsi";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("node", "a.dts", make_range((3, 1), (3, 5))),
                ("node", "b.dts", make_range((2, 1), (2, 5))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![("node", "common.dtsi", make_range((2, 10), (2, 14)))],
        )
        .await;
        assert!(be.data.rd == expected_rd);

        let res = be.mock_rename_prepare(path, Position::new(2, 10)).await;
        assert!(res.is_ok());

        let res = be.mock_rename(path, Position::new(2, 10), "changed").await;
        let mut expected = Changes::new();
        expected.add_edit("a.dts", (3, 1), (3, 5), "changed");
        expected.add_edit("b.dts", (2, 1), (2, 5), "changed");
        expected.add_edit("common.dtsi", (2, 10), (2, 14), "changed");
        assert_eq!(expected.0, res);

        assert_eq!(be.data.fd.size().await, 3);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 1);

        let expected_ld = LabelsDepot::new_expected(
            &be.data.fd,
            vec![
                ("changed", "a.dts", make_range((3, 1), (3, 8))),
                ("changed", "b.dts", make_range((2, 1), (2, 8))),
            ],
        )
        .await;
        assert!(be.data.ld == expected_ld);

        #[rustfmt::skip]
        let expected_rd = ReferencesDepot::new_expected(
            &be.data.fd,
            vec![
            ("changed", "common.dtsi", make_range((2, 10), (2, 17))),
            ],
        ).await;
        assert!(be.data.rd == expected_rd);
    }
    {
        info!("TEST 10");
        let be = &make_ctx("tests/5/").0;
        let path = "before.dts";

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
            let expected_file = format!("after-{index}.dts");
            be.test_edit(&make_url(path), edit.clone(), &expected_file)
                .await;
        }
    }
    {
        info!("TEST 11");
        let be = &make_ctx("tests/5/").0;
        let path = "before.dts";

        be.mock_open(path).await;

        let _ = be.mock_rename(path, Position::new(1, 1), "lbl").await;
        assert!(be.verify_file(&make_url(path), "after-2.dts").await);

        let _ = be
            .mock_rename(path, Position::new(1, 1), "some_label")
            .await;
        assert!(be.verify_file(&make_url(path), "before.dts").await);

        let _ = be
            .mock_rename(path, Position::new(1, 1), "very_long_label_value")
            .await;
        assert!(be.verify_file(&make_url(path), "after-4.dts").await);
    }
    {
        info!("TEST 12");
        // find references breaks after buffer is changed and restored
        let be = &make_ctx("tests/2/").0;
        let path = "a.dts";
        let uri = make_url(path);

        be.mock_open(path).await;
        be.data.fd.apply_edits(&uri, &vec![]);
        let res = be.mock_refrences(path, Position::new(3, 1)).await;
        assert_eq!(res.unwrap().unwrap().len(), 1);

        let old_file_contents = be.data.fd.get_text(&uri).unwrap();
        be.mock_change(path, String::new()).await;
        be.mock_change(path, old_file_contents).await;

        let res = be.mock_refrences(path, Position::new(3, 1)).await;
        info!("{}", be.data.fd.get_text(&make_url(path)).unwrap());
        assert_eq!(res.unwrap().unwrap().len(), 1);
    }
    {
        info!("TEST 13");
        let be = &make_ctx("tests/apply_edits/").0;
        let path = "file.dts";
        let uri = make_url(path);

        let change1 = read_to_string("change1.txt").unwrap();
        let change2 = read_to_string("change2.txt").unwrap();

        let edits = vec![
            TextEdit::new(make_range((6, 1), (6, 13)), change2),
            TextEdit::new(make_range((4, 1), (5, 3)), change1),
            TextEdit::new(make_range((1, 1), (1, 9)), "n1".to_string()),
        ];

        be.mock_open(path).await;
        be.data.fd.apply_edits(&uri, &edits);

        assert!(be.verify_file(&uri, "expected.dts").await);
    }
    {
        info!("TEST 14");
        let be = &make_ctx("tests/includes_with_prefix/").0;
        let path = "good_file.dts";

        be.mock_open(path).await;

        let pos = Position::new(9, 22); // LOCAL
        let res = be.mock_goto_definition(path, pos).await;
        let loc = Location::new(make_url("local.h"), make_range((0, 8), (0, 13)));
        assert_eq!(res.unwrap().unwrap(), GotoDefinitionResponse::Scalar(loc));

        be.data.fd.dump();

        let pos = Position::new(9, 13); // VALUE
        let res = be.mock_goto_definition(path, pos).await;
        let loc = Location::new(
            make_url("include/dt-bindings/test/test.h"),
            make_range((0, 8), (0, 13)),
        );
        assert_eq!(res.unwrap().unwrap(), GotoDefinitionResponse::Scalar(loc));

        let pos = Position::new(9, 30); // INPLACE
        let res = be.mock_goto_definition(path, pos).await;
        let loc = Location::new(make_url(path), make_range((4, 8), (4, 15)));
        assert_eq!(res.unwrap().unwrap(), GotoDefinitionResponse::Scalar(loc));
    }
}
