---
title: Syntax Highlighting
description: Code block syntax highlighting with 80+ languages
---

# Syntax Highlighting

undox provides syntax highlighting for 80+ programming languages using [tree-sitter](https://tree-sitter.github.io/tree-sitter/), the same parser used by GitHub, Neovim, and other modern editors.

## Usage

Add a language identifier after the opening fence:

````markdown
```rust
fn main() {
    println!("Hello, world!");
}
```
````

Renders as:

```rust
fn main() {
    println!("Hello, world!");
}
```

## Supported Languages

### Web Development

```javascript
// JavaScript
const greeting = (name) => `Hello, ${name}!`;
export default greeting;
```

```typescript
// TypeScript
interface User {
  name: string;
  age: number;
}

const greet = (user: User): string => `Hello, ${user.name}!`;
```

```html
<!-- HTML -->
<!DOCTYPE html>
<html>
  <head><title>Hello</title></head>
  <body>
    <h1>Welcome</h1>
  </body>
</html>
```

```css
/* CSS */
.container {
  display: flex;
  justify-content: center;
  background: linear-gradient(to right, #667eea, #764ba2);
}
```

### Systems Programming

```rust
// Rust
use std::collections::HashMap;

fn main() {
    let mut scores: HashMap<&str, i32> = HashMap::new();
    scores.insert("Blue", 10);
    println!("{:?}", scores);
}
```

```go
// Go
package main

import "fmt"

func main() {
    messages := make(chan string)
    go func() { messages <- "ping" }()
    fmt.Println(<-messages)
}
```

### Scripting

```python
# Python
def fibonacci(n: int) -> list[int]:
    fib = [0, 1]
    for i in range(2, n):
        fib.append(fib[i-1] + fib[i-2])
    return fib

print(fibonacci(10))
```

```bash
#!/bin/bash
# Bash
for file in *.md; do
    echo "Processing $file"
    wc -l "$file"
done
```

### Data Formats

```json
{
  "name": "undox",
  "version": "0.1.0",
  "features": ["markdown", "syntax-highlighting", "multi-repo"]
}
```

```yaml
# YAML
site:
  name: "My Docs"
  url: "https://example.com"

sources:
  - name: docs
    path: ./content
```

## Full Language List

undox supports these languages (and their common aliases):

| Language | Identifiers |
|----------|-------------|
| Bash | `bash`, `sh`, `shell` |
| C | `c` |
| C++ | `cpp`, `c++` |
| C# | `csharp`, `cs` |
| CSS | `css` |
| Diff | `diff` |
| Elixir | `elixir` |
| Erlang | `erlang` |
| Go | `go`, `golang` |
| Haskell | `haskell`, `hs` |
| HTML | `html` |
| Java | `java` |
| JavaScript | `javascript`, `js` |
| JSON | `json` |
| Kotlin | `kotlin`, `kt` |
| Lua | `lua` |
| Markdown | `markdown`, `md` |
| Nix | `nix` |
| OCaml | `ocaml` |
| PHP | `php` |
| Python | `python`, `py` |
| Ruby | `ruby`, `rb` |
| Rust | `rust`, `rs` |
| Scala | `scala` |
| SQL | `sql` |
| Swift | `swift` |
| TypeScript | `typescript`, `ts` |
| TSX | `tsx` |
| XML | `xml` |
| YAML | `yaml`, `yml` |
| Zig | `zig` |

...and 50+ more!

## No Language Specified

Code blocks without a language identifier are rendered without highlighting:

```
This is plain text
No syntax highlighting applied
```

## Theme

Syntax highlighting uses a GitHub Dark inspired color scheme that works well on both light and dark backgrounds.
