use assert_cmd::Command;
use predicates::prelude::*;

macro_rules! cmd_success_tests {
    ($($name:ident: $value:expr,)*) => {
    $(
        #[test]
        fn $name(){
            let (stdin, args, expected) = $value;
            Command::cargo_bin("hq")
                .unwrap()
                .args(args)
                .write_stdin(stdin)
                .assert()
                .success()
                .stdout(predicate::str::diff(expected));
        }
    )*
    }
}

cmd_success_tests!(
    find_by_class: (
        "<html><head></head><body><div class=\"hi\"><a href=\"/foo/bar\">Hello</a></div></body></html>",
        [".hi"],
        "<div class=\"hi\"><a href=\"/foo/bar\">Hello</a></div>\n"
    ),
    find_by_id: (
        "<html><head></head><body><div id=\"my-id\"><a href=\"/foo/bar\">Hello</a></div></body></html>",
        ["#my-id"],
        "<div id=\"my-id\"><a href=\"/foo/bar\">Hello</a></div>\n"
    ),
    remove_links: (
        "<html><head></head><body><div id=\"my-id\"><a href=\"/foo/bar\">Hello</a></div></body></html>",
        ["#my-id", "--remove-nodes", "a"],
        "<div id=\"my-id\"></div>\n",
    ),
    compact_json_preserves_spaces: (
        "<html><body><script type=\"application/ld+json\">\n{\n  \"title\": \"Business Development Manager, Supply\",\n  \"company\": \"Acme Corp\"\n}\n</script></body></html>",
        ["script", "-t", "-c"],
        "{\"company\":\"Acme Corp\",\"title\":\"Business Development Manager, Supply\"}"
    ),
    compact_json_multiple_values: (
        "<html><body><script>\n{\n  \"name\": \"John Doe\",\n  \"role\": \"Senior Software Engineer\",\n  \"location\": \"New York City\"\n}\n</script></body></html>",
        ["script", "-t", "--compact"],
        "{\"location\":\"New York City\",\"name\":\"John Doe\",\"role\":\"Senior Software Engineer\"}"
    ),
    compact_json_nested: (
        "<html><body><script>\n{\n  \"person\": {\n    \"name\": \"Jane Smith\",\n    \"age\": 30\n  }\n}\n</script></body></html>",
        ["script", "-t", "-c"],
        "{\"person\":{\"age\":30,\"name\":\"Jane Smith\"}}"
    ),
    compact_json_array: (
        "<html><body><script>\n[\n  \"First Item\",\n  \"Second Item\",\n  \"Third Item\"\n]\n</script></body></html>",
        ["script", "-t", "-c"],
        "[\"First Item\",\"Second Item\",\"Third Item\"]"
    ),
    compact_html_minifies: (
        "<html><head>  <title>Test</title>  </head><body>  <div>  <p>Text</p>  </div>  </body></html>",
        ["body", "-c"],
        "<body><div><p>Text</p></div></body>"
    ),
    compact_text_plain: (
        "<html><body><div>  \n  Hello World  \n  </div></body></html>",
        ["div", "-t", "-c"],
        "Hello World  \n  \n"
    ),
    without_compact_preserves_whitespace: (
        "<html><body><script>\n{\n  \"title\": \"Test\"\n}\n</script></body></html>",
        ["script", "-t"],
        "\n{\n  \"title\": \"Test\"\n}\n\n"
    ),
);
