# mdserve

A tiny local web server for browsing Markdown files. It builds a live index from `.md` and `.markdown` files, including nested folders, and renders each document as a clean HTML page. The interface follows your operating system's light or dark appearance and uses a modern system typeface stack.

## Run

```sh
cargo run -- /path/to/your/markdown
```

Then open [http://127.0.0.1:8345](http://127.0.0.1:8345). With no folder argument, mdserve uses the current directory. An optional second argument changes the listen address:

```sh
cargo run -- ./notes 127.0.0.1:9000
```

The server binds to localhost by default. Markdown is rendered with tables, task lists, and strikethrough support. Raw HTML is omitted and unsafe link schemes are disabled.
