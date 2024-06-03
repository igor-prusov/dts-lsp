use std::path::Path;

impl Backend {
    async fn mock_open(&self, uri: &str) {
        let mut file = File::open(uri).unwrap();
        let mut file_data = String::new();

        file.read_to_string(&mut file_data).unwrap();

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
}

fn make_url(path: &str) -> Url {
    let path = Path::new(path).canonicalize().unwrap();
    Url::from_file_path(path).unwrap()
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
        let be = Backend::new();
        let path = "tests/2/b.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 3);
        assert_eq!(be.data.ld.size().await, 2);
        assert_eq!(be.data.rd.size().await, 1);

        assert_eq!(be.has_label(path, "node").await, 2);
    }
    {
        let be = Backend::new();
        let path = "tests/3/a.dts";

        be.mock_open(path).await;

        assert_eq!(be.data.fd.size().await, 1);
        assert_eq!(be.data.ld.size().await, 2);

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
}
