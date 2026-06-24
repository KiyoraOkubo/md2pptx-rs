# md2pptx Sample

Markdown から PPTX を書き出す最小サンプルです。**太字**、*斜体*、`inline code` を含みます。

- スライドは `---` で分割
- 箇条書きに対応
- レイアウト超過は警告として表示

---

# Code and Quote

> 引用ブロックもテキストボックスとして出力します。

```rust
fn main() {
    println!("hello pptx");
}
```

---

# Next Steps

1. 画像は Markdown ファイルからの相対パスで解決
2. Mermaid と数式は未対応エラー
3. 複雑なレイアウトは今後追加
