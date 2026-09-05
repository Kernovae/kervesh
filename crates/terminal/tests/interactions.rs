use kervesh_terminal::Terminal;
#[test]
fn search_finds_scrollback_wraps_and_combining_without_pty_writes() {
    let mut terminal = Terminal::new(8, 2, 100);
    terminal.feed("abcHELLOworld\r\ne\u{301}cho\r\nlast\r\n".as_bytes());
    terminal.search("helloworld", false);
    assert_eq!(terminal.search_matches().len(), 1);
    terminal.search("HELLOworld", true);
    assert_eq!(terminal.search_matches().len(), 1);
    terminal.search("e\u{301}", true);
    assert_eq!(terminal.search_matches().len(), 1);
    assert!(terminal.replies().is_empty());
}
#[test]
fn links_require_safe_schemes_and_osc7_metadata_is_streaming() {
    let mut terminal = Terminal::new(80, 24, 100);
    terminal.feed(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
    assert_eq!(
        terminal.hyperlink_at(0, 1).as_deref(),
        Some("https://example.com")
    );
    terminal.feed(b"\r\nhttps://example.org/path");
    assert_eq!(
        terminal.hyperlink_at(1, 10).as_deref(),
        Some("https://example.org/path")
    );
    terminal.feed(b"\r\n\x1b]8;;javascript:alert(1)\x1b\\bad\x1b]8;;\x1b\\");
    assert_eq!(terminal.hyperlink_at(2, 1), None);
    terminal.feed(b"\x1b]7;file://server/tmp/my%20");
    assert!(terminal.directory().is_none());
    terminal.feed(b"dir\x1b\\");
    assert_eq!(terminal.directory().unwrap().path, "/tmp/my dir");
    terminal.feed(b"\x1b]7;file://server/tmp/%00bad\x07");
    assert!(terminal.directory().is_none());
}
#[test]
fn selection_uses_alacritty_soft_wrap_and_semantic_line_rules() {
    let mut terminal = Terminal::new(8, 3, 100);
    terminal.feed(b"hello world\r\nlast");
    terminal.select_range((0, 0), (1, 2));
    assert_eq!(terminal.selection_text().as_deref(), Some("hello world"));
    terminal.select_word(0, 2);
    assert_eq!(terminal.selection_text().as_deref(), Some("hello"));
    terminal.select_line(0);
    assert_eq!(terminal.selection_text().as_deref(), Some("hello world\n"));
}
