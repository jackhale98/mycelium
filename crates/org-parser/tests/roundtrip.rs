use org_parser::{parse, serialize};

fn assert_roundtrip(input: &str) {
    let doc = parse(input);
    let output = serialize::serialize(&doc);
    assert_eq!(
        input, output,
        "\n--- EXPECTED ---\n{}\n--- GOT ---\n{}\n",
        input, output
    );
}

#[test]
fn roundtrip_simple_heading() {
    assert_roundtrip("* Hello World\n");
}

#[test]
fn roundtrip_metadata_and_heading() {
    assert_roundtrip("#+TITLE: Test\n* Heading\nSome text.\n");
}

#[test]
fn roundtrip_nested_headings() {
    assert_roundtrip(
        "* Level 1\n** Level 2\n*** Level 3\nBody text.\n",
    );
}

#[test]
fn roundtrip_properties() {
    assert_roundtrip(
        "* My Node\n:PROPERTIES:\n:ID: abc-123\n:END:\nBody.\n",
    );
}

#[test]
fn roundtrip_todo_and_tags() {
    assert_roundtrip("* TODO [#A] Urgent task :work:urgent:\n");
}

#[test]
fn roundtrip_planning() {
    assert_roundtrip(
        "* TODO Task\nSCHEDULED: <2024-01-15 Mon>\nBody.\n",
    );
}

#[test]
fn roundtrip_list() {
    assert_roundtrip("* Heading\n- Item one\n- Item two\n- Item three\n");
}

#[test]
fn roundtrip_checkbox_list() {
    assert_roundtrip("* Tasks\n- [ ] Todo\n- [X] Done\n");
}

#[test]
fn roundtrip_src_block() {
    assert_roundtrip(
        "* Code\n#+BEGIN_SRC python\nprint(\"hello\")\n#+END_SRC\n",
    );
}

#[test]
fn roundtrip_table() {
    assert_roundtrip(
        "* Data\n| A | B |\n|---+---|\n| 1 | 2 |\n",
    );
}

#[test]
fn roundtrip_links() {
    assert_roundtrip(
        "* Node\n:PROPERTIES:\n:ID: src\n:END:\nLink to [[id:target][Target]].\n",
    );
}

#[test]
fn roundtrip_markup() {
    assert_roundtrip("* Text\nThis is *bold* and /italic/ and ~code~.\n");
}

#[test]
fn roundtrip_filetags() {
    assert_roundtrip("#+TITLE: Test\n#+FILETAGS: :tag1:tag2:\n* Heading\n");
}

#[test]
fn roundtrip_blank_lines() {
    assert_roundtrip("#+TITLE: Test\n\n* Heading\n\nSome text.\n\n** Sub\n");
}

#[test]
fn roundtrip_file_level_properties() {
    assert_roundtrip(":PROPERTIES:\n:ID: file-id-123\n:ROAM_ALIASES: \"Alias\"\n:END:\n#+TITLE: My Note\n* Heading\nBody.\n");
}

#[test]
fn roundtrip_file_level_with_preamble() {
    assert_roundtrip(":PROPERTIES:\n:ID: abc\n:END:\n#+TITLE: Test\n\nSome preamble text.\n\n* Heading\n");
}
