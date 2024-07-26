use utils::current_url;

async fn make_backend_ext(path: &str, process_neighbours: bool) -> Backend {
    // Go to test directory, each test directory emulates a workspace
    let be = Backend {
        data: Data::new(),
        process_neighbours,
        client: None,
    };
    let mut root = current_url().unwrap();
    let root_path = root.path().to_string() + "/" + path.trim_end_matches('/');
    root.set_path(&root_path);
    be.mock_initialize(root).await;
    be
}

async fn make_backend(path: &str) -> Backend {
    make_backend_ext(path, true).await
}

impl Backend {
    async fn mock_initialize(&self, uri: Url) {
        let params = InitializeParams {
            root_uri: Some(uri),
            ..Default::default()
        };
        self.initialize(params).await.unwrap();
    }

    async fn mock_open(&self, uri: &str) {
        let prefix = self.data.fd.get_root_dir().unwrap();
        let prefix = prefix.join(uri).unwrap();
        let path = prefix.to_file_path().unwrap();
        let file_data = read_to_string(path).unwrap();

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(prefix, "dts".to_owned(), 1, file_data),
        };

        self.did_open(params).await;
    }

    async fn mock_change(&self, uri: &str, file_data: String) {
        let prefix = self.data.fd.get_root_dir().unwrap();
        let uri = prefix.join(uri).unwrap();

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

    fn make_url(&self, path: &str) -> Url {
        let prefix = self.data.fd.get_root_dir().unwrap();
        prefix.join(path).unwrap()
    }

    fn has_label(&self, path: &str, label: &str) -> usize {
        self.data
            .ld
            .find_label(&self.data.fd.make_url(path), label)
            .len()
    }

    async fn mock_rename_prepare(
        &self,
        uri: &str,
        pos: Position,
    ) -> Result<Option<PrepareRenameResponse>> {
        let prefix = self.data.fd.get_root_dir().unwrap();
        let uri = prefix.join(uri).unwrap();
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
        let prefix = self.data.fd.get_root_dir().unwrap();
        let uri = prefix.join(uri).unwrap();
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
        let prefix = self.data.fd.get_root_dir().unwrap();
        let uri = prefix.join(uri).unwrap();
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
        let prefix = self.data.fd.get_root_dir().unwrap();
        let uri = prefix.join(uri).unwrap();
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

    fn verify_file(&self, uri: &Url, expected_uri: &Url) -> bool {
        let text = self.data.fd.get_text(uri).unwrap();
        let expected_path = expected_uri.to_file_path().unwrap();
        let expected_text = read_to_string(expected_path).unwrap();
        let res = text == expected_text;

        if !res {
            error!("---\ngot:\n{}\nexpected:\n{}\n---\n", text, expected_text);
        }

        res
    }
    fn test_edit(&self, uri: &Url, edit: TextEdit, expected_file: &str) {
        self.data.fd.apply_edits(uri, &vec![edit]);
        let expected_uri = self.data.fd.get_root_dir().unwrap();
        let expected_uri = expected_uri.join(expected_file).unwrap();
        let res = self.verify_file(uri, &expected_uri);
        assert!(res);
    }

    fn verify_labels(&self, data: Vec<(&str, &str, Range)>) -> bool {
        let ld = LabelsDepot::new(&self.data.fd);
        for x in data {
            ld.add_label(x.0, &self.data.fd.make_url(x.1), x.2);
        }
        ld == self.data.ld
    }

    fn verify_references(&self, data: Vec<(&str, &str, Range)>) -> bool {
        let rd = ReferencesDepot::new(&self.data.fd);
        for x in data {
            rd.add_reference(x.0, &self.data.fd.make_url(x.1), x.2);
        }
        rd == self.data.rd
    }
}

fn make_range(begin: (u32, u32), end: (u32, u32)) -> Range {
    let begin = Position::new(begin.0, begin.1);
    let end = Position::new(end.0, end.1);
    Range::new(begin, end)
}

impl FileDepot {
    fn make_url(&self, path: &str) -> Url {
        let prefix = self.get_root_dir().unwrap();
        prefix.join(path).unwrap()
    }
}

#[derive(Debug, PartialEq)]
struct Changes(Result<Option<WorkspaceEdit>>, Url);

impl Changes {
    // TODO: add new_expected
    fn new(path: Url) -> Self {
        Self(Ok(Some(WorkspaceEdit::new(HashMap::new()))), path)
    }
    fn add_edit(&mut self, uri: &str, begin: (u32, u32), end: (u32, u32), new_text: &str) {
        let uri = self.1.join(uri).unwrap();

        let x = self.0.as_mut().unwrap().as_mut().unwrap();
        let changes = x.changes.get_or_insert(HashMap::new());

        changes.entry(uri).or_default().push(TextEdit {
            range: Range::new(Position::new(begin.0, begin.1), Position::new(end.0, end.1)),
            new_text: new_text.to_string(),
        });
    }
}

use super::*;

#[tokio::test]
async fn open_0() {
    let be = &make_backend_ext("tests/1/", false).await;
    let path = "bad_file.dts";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 1);
    assert_eq!(be.data.ld.size(), 0);
    assert_eq!(be.data.rd.size(), 0);
}

#[tokio::test]
async fn open_1() {
    let be = &make_backend("tests/1/").await;
    let path = "good_file.dts";

    be.mock_open(path).await;

    be.data.fd.dump();
    assert_eq!(be.data.fd.size(), 2);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 0);

    assert_eq!(be.has_label(path, "a"), 1);
    assert_eq!(be.has_label(path, "b"), 1);
    assert_eq!(be.has_label(path, "label"), 0);
}

#[tokio::test]
async fn open_2() {
    let be = &make_backend("tests/1/").await;
    let bad_path = "bad_file.dts";
    let good_path = "good_file.dts";

    be.mock_open(bad_path).await;

    assert_eq!(be.data.fd.size(), 2);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 0);

    assert_eq!(be.has_label(good_path, "a"), 1);
    assert_eq!(be.has_label(good_path, "b"), 1);
    assert_eq!(be.has_label(good_path, "label"), 0);
}

#[tokio::test]
async fn open_3() {
    /* Files without supported extension should be ignored */
    let be = &make_backend("tests/1/").await;
    let bad_ext_path = "good_file.bad_ext";
    let good_path = "good_file.dts";

    be.mock_open(bad_ext_path).await;

    assert_eq!(be.data.fd.size(), 2);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 0);

    assert_eq!(be.has_label(good_path, "a"), 1);
    assert_eq!(be.has_label(good_path, "b"), 1);
    assert_eq!(be.has_label(good_path, "label"), 0);
}

#[tokio::test]
async fn open_4() {
    let be = &make_backend("tests/2/").await;
    let path = "b.dts";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 3);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 1);

    assert_eq!(be.has_label(path, "node"), 1);
}

#[tokio::test]
async fn open_5() {
    let be = &make_backend("tests/3/").await;
    let path = "a.dts";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 1);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 0);

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
    assert_eq!(be.data.fd.size(), 1);
    assert_eq!(be.data.ld.size(), 1);
}

#[tokio::test]
async fn open_6() {
    /* File including non-existent DTSI file */
    let be = &make_backend("tests/4/").await;
    let path = "a.dts";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 2);
    assert_eq!(be.data.fd.n_with_text(), 1);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 0);
}

#[tokio::test]
async fn rename_0() {
    let be = &make_backend("tests/1/").await;
    let path = "good_file.dts";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 2);
    assert_eq!(be.data.fd.n_with_text(), 2);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 0);

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

    let mut expected = Changes::new(be.data.fd.get_root_dir().unwrap());
    expected.add_edit(path, (1, 1), (1, 2), "changed");
    let res = be.mock_rename(path, Position::new(1, 1), "changed").await;
    assert_eq!(expected.0, res);
}

#[tokio::test]
async fn rename_1() {
    // TODO: test single file, multiple references
    // TODO: probably b.dts should change as well
    let be = &make_backend("tests/2/").await;
    let path = "a.dts";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 3);

    assert!(be.verify_labels(vec![
        ("node", "a.dts", make_range((3, 1), (3, 5))),
        ("node", "b.dts", make_range((2, 1), (2, 5))),
    ]));

    #[rustfmt::skip]
    assert!(be.verify_references(vec![
        ("node", "common.dtsi", make_range((2, 10), (2, 14)))
    ]));

    let res = be.mock_rename_prepare(path, Position::new(3, 1)).await;
    assert!(res.is_ok());

    let res = be.mock_rename(path, Position::new(3, 1), "changed").await;
    let mut expected = Changes::new(be.data.fd.get_root_dir().unwrap());
    expected.add_edit("a.dts", (3, 1), (3, 5), "changed");
    expected.add_edit("common.dtsi", (2, 10), (2, 14), "changed");
    assert_eq!(expected.0, res);

    assert_eq!(be.data.fd.size(), 3);

    assert!(be.verify_labels(vec![
        ("changed", "a.dts", make_range((3, 1), (3, 8))),
        ("node", "b.dts", make_range((2, 1), (2, 5))),
    ]));

    #[rustfmt::skip]
    assert!(be.verify_references(vec![
        ("changed", "common.dtsi", make_range((2, 10), (2, 17)))
    ]));
}

#[tokio::test]
async fn rename_2() {
    let be = &make_backend("tests/2/").await;
    let path = "common.dtsi";

    be.mock_open(path).await;

    assert_eq!(be.data.fd.size(), 3);

    assert!(be.verify_labels(vec![
        ("node", "a.dts", make_range((3, 1), (3, 5))),
        ("node", "b.dts", make_range((2, 1), (2, 5))),
    ]));

    #[rustfmt::skip]
    assert!(be.verify_references(vec![
        ("node", "common.dtsi", make_range((2, 10), (2, 14))),
    ]));

    let res = be.mock_rename_prepare(path, Position::new(2, 10)).await;
    assert!(res.is_ok());

    let res = be.mock_rename(path, Position::new(2, 10), "changed").await;
    let mut expected = Changes::new(be.data.fd.get_root_dir().unwrap());
    expected.add_edit("a.dts", (3, 1), (3, 5), "changed");
    expected.add_edit("b.dts", (2, 1), (2, 5), "changed");
    expected.add_edit("common.dtsi", (2, 10), (2, 14), "changed");
    assert_eq!(expected.0, res);

    assert_eq!(be.data.fd.size(), 3);
    assert_eq!(be.data.ld.size(), 2);
    assert_eq!(be.data.rd.size(), 1);

    assert!(be.verify_labels(vec![
        ("changed", "a.dts", make_range((3, 1), (3, 8))),
        ("changed", "b.dts", make_range((2, 1), (2, 8))),
    ]));

    #[rustfmt::skip]
    assert!(be.verify_references(vec![
        ("changed", "common.dtsi", make_range((2, 10), (2, 17)))
    ]));
}

#[tokio::test]
async fn fiel_edits_0() {
    let be = &make_backend("tests/5/").await;
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
        be.test_edit(&be.make_url(path), edit.clone(), &expected_file);
    }
}

#[tokio::test]
async fn rename_3() {
    let be = &make_backend("tests/5/").await;
    let path = "before.dts";

    be.mock_open(path).await;

    let _ = be.mock_rename(path, Position::new(1, 1), "lbl").await;
    assert!(be.verify_file(&be.make_url(path), &be.make_url("after-2.dts")));

    let _ = be
        .mock_rename(path, Position::new(1, 1), "some_label")
        .await;
    assert!(be.verify_file(&be.make_url(path), &be.make_url("before.dts")));

    let _ = be
        .mock_rename(path, Position::new(1, 1), "very_long_label_value")
        .await;
    assert!(be.verify_file(&be.make_url(path), &be.make_url("after-4.dts")));
}

#[tokio::test]
async fn references_0() {
    // find references breaks after buffer is changed and restored
    let be = &make_backend("tests/2/").await;
    let path = "a.dts";
    let uri = be.make_url(path);

    be.mock_open(path).await;
    be.data.fd.apply_edits(&uri, &vec![]);
    let res = be.mock_refrences(path, Position::new(3, 1)).await;
    assert_eq!(res.unwrap().unwrap().len(), 1);

    let old_file_contents = be.data.fd.get_text(&uri).unwrap();
    be.mock_change(path, String::new()).await;
    be.mock_change(path, old_file_contents).await;

    let res = be.mock_refrences(path, Position::new(3, 1)).await;
    info!("{}", be.data.fd.get_text(&be.make_url(path)).unwrap());
    assert_eq!(res.unwrap().unwrap().len(), 1);
}

#[tokio::test]
async fn apply_edits_0() {
    let be = &make_backend("tests/apply_edits/").await;
    let path = "file.dts";
    let uri = be.make_url(path);

    let change1 = read_to_string(be.make_url("change1.txt").to_file_path().unwrap()).unwrap();
    let change2 = read_to_string(be.make_url("change2.txt").to_file_path().unwrap()).unwrap();

    let edits = vec![
        TextEdit::new(make_range((6, 1), (6, 13)), change2),
        TextEdit::new(make_range((4, 1), (5, 3)), change1),
        TextEdit::new(make_range((1, 1), (1, 9)), "n1".to_string()),
    ];

    be.mock_open(path).await;
    be.data.fd.apply_edits(&uri, &edits);

    assert!(be.verify_file(&uri, &be.make_url("expected.dts")));
}

#[tokio::test]
async fn goto_definition_0() {
    let be = &make_backend("tests/includes_with_prefix/").await;
    let path = "good_file.dts";

    be.mock_open(path).await;

    let pos = Position::new(9, 22); // LOCAL
    let res = be.mock_goto_definition(path, pos).await;
    let loc = Location::new(be.make_url("local.h"), make_range((0, 8), (0, 13)));
    assert_eq!(res.unwrap().unwrap(), GotoDefinitionResponse::Scalar(loc));

    be.data.fd.dump();
    be.data.id.dump();

    let pos = Position::new(9, 13); // VALUE
    let res = be.mock_goto_definition(path, pos).await;
    let loc = Location::new(
        be.make_url("include/dt-bindings/test/test.h"),
        make_range((0, 8), (0, 13)),
    );
    assert_eq!(res.unwrap().unwrap(), GotoDefinitionResponse::Scalar(loc));

    let pos = Position::new(9, 30); // INPLACE
    let res = be.mock_goto_definition(path, pos).await;
    let loc = Location::new(be.make_url(path), make_range((4, 8), (4, 15)));
    assert_eq!(res.unwrap().unwrap(), GotoDefinitionResponse::Scalar(loc));
}
