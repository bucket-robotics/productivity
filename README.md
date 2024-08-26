# Productivity Tools

Bucket's productivity tools.

## `ask` CLI agent

Tool to automate things using Anthropic.

```
Usage: ask [-v] [question...]

Ask a question.

Options:
  -v, --verbose     verbose mode
  --help            display usage information
```

### Installation

```bash
# To install from GitHub
cargo install --git https://github.com/bucket-robotics/productivity.git bucket_ask

# To install from a local clone of the repo
cargo install --path ./ask
```

### Example

![ask example](examples/ask.svg)

## `go/` links

Tool to find and open `go/` links from [OrgOrg](orgorg.us).

```
Usage: golink [<link>] [--print] [--json]

CLI to access `go/` links.

Positional Arguments:
  link              the link to open

Options:
  --print           print the link instead of opening it
  --json            print the query result as JSON
  --help            display usage information
```

### Installation

```bash
# To install from GitHub
cargo install --git https://github.com/bucket-robotics/productivity.git golink

# To install from a local clone of the repo
cargo install --path ./golink
```

### Example

![search example](examples/search.svg)
