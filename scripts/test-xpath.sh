#!/usr/bin/env bash
# Test Feader XPath selectors against local HTML without needing the Tauri app.
# Usage: test-xpath.sh <html-file> <xpath-expression>
#        test-xpath.sh <html-file> --selectors <selectors.json>
set -euo pipefail

HTML_FILE="${1:-}"
EXPRESSION="${2:-}"

usage() {
  cat <<'HELP'
test-xpath.sh — test XPath expressions against local HTML files.

Usage:
  # Single expression
  test-xpath.sh page.html "//article"

  # Count matches
  test-xpath.sh page.html "count(//article)"

  # Test a full selectors.json file
  test-xpath.sh page.html --selectors selectors.json

  # Pipe HTML from curl
  curl -s https://example.com | test-xpath.sh - "//h1"

Dependencies: xmllint (built into macOS)
HELP
  exit 0
}

# --- Argument parsing ---
SELECTORS_FILE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --selectors) SELECTORS_FILE="$2"; shift 2 ;;
    --help|-h) usage ;;
    *)
      if [[ -z "$HTML_FILE" ]]; then
        HTML_FILE="$1"
        shift
      elif [[ -z "$EXPRESSION" && -z "$SELECTORS_FILE" ]]; then
        EXPRESSION="$1"
        shift
      else
        echo "Unknown argument: $1" >&2
        exit 1
      fi
      ;;
  esac
done

if [[ -z "$HTML_FILE" ]]; then
  echo "Error: HTML file or '-' (stdin) is required" >&2
  usage
fi

# Read HTML content
if [[ "$HTML_FILE" == "-" ]]; then
  HTML=$(cat)
else
  if [[ ! -r "$HTML_FILE" ]]; then
    echo "Error: cannot read file: $HTML_FILE" >&2
    exit 1
  fi
  HTML=$(cat "$HTML_FILE")
fi

# Normalize HTML to XHTML so xmllint can process it
normalize_for_xpath() {
  # Use macOS built-in textutil for HTML→XHTML, fall back to basic sed cleanup
  local tmp_file
  tmp_file=$(mktemp)
  echo "$HTML" > "$tmp_file"

  # Try textutil first (macOS built-in, handles real-world HTML well)
  if command -v textutil &>/dev/null; then
    textutil -convert html -output "$tmp_file.xhtml" "$tmp_file" 2>/dev/null || true
    if [[ -f "$tmp_file.xhtml" ]]; then
      cat "$tmp_file.xhtml"
      rm -f "$tmp_file.xhtml" "$tmp_file"
      return
    fi
  fi

  # Fallback: basic sed cleanup for xmllint compatibility
  echo "$HTML" \
    | sed 's/<!DOCTYPE[^>]*>//i' \
    | sed 's/<script[^>]*>.*<\/script>//gi' \
    | sed 's/<noscript[^>]*>.*<\/noscript>//gi' \
    | sed 's/<style[^>]*>.*<\/style>//gi'
  rm -f "$tmp_file"
}

evaluate_xpath() {
  local xpath="$1"
  local html
  html=$(normalize_for_xpath)

  # xmllint --xpath returns results; --html handles tag soup
  # Use --html for tag-soup input, pipe through xmllint again for XPath
  echo "$html" | xmllint --html --xpath "$xpath" - 2>/dev/null || echo "(no match)"
}

print_field_status() {
  local field="$1"
  local xpath="$2"
  local required="$3"
  local result
  result=$(evaluate_xpath "$xpath")
  if [[ "$result" == "(no match)" ]]; then
    echo "  [$field] ${required:+REQUIRED }MISSING: $xpath"
  else
    local count
    count=$(echo "$result" | grep -c '<' 2>/dev/null || echo "?")
    echo "  [$field] OK: $xpath → $(echo "$result" | head -c 80)..."
  fi
}

# --- Single expression mode ---
if [[ -n "$EXPRESSION" ]]; then
  evaluate_xpath "$EXPRESSION"
  exit 0
fi

# --- Selectors file mode ---
if [[ -n "$SELECTORS_FILE" ]]; then
  if [[ ! -r "$SELECTORS_FILE" ]]; then
    echo "Error: cannot read selectors file: $SELECTORS_FILE" >&2
    exit 1
  fi

  echo "=== XPath Selector Diagnostics ==="

  ITEMS=$(jq -r '.items // ""' "$SELECTORS_FILE" 2>/dev/null || echo "")
  if [[ -n "$ITEMS" ]]; then
    COUNT=$(evaluate_xpath "count($ITEMS)")
    echo "  [items]  $ITEMS → $COUNT items found"
  fi

  for field in title url summary publishedAt author content detailContent image nextPage; do
    XPATH=$(jq -r ".$field // empty" "$SELECTORS_FILE" 2>/dev/null || echo "")
    if [[ -n "$XPATH" ]]; then
      REQUIRED=""
      [[ "$field" == "title" || "$field" == "url" ]] && REQUIRED="REQUIRED "
      print_field_status "$field" "$XPATH" "$REQUIRED"
    fi
  done

  # Custom fields
  jq -r '.customFields[]? | "\(.key)\t\(.xpath)\t\(.scope // "item")"' "$SELECTORS_FILE" 2>/dev/null | while IFS=$'\t' read -r key xpath scope; do
    if [[ -n "$key" && -n "$xpath" ]]; then
      echo "  [custom:$key] scope=$scope: $xpath"
    fi
  done

  echo ""
  echo "=== First Article Preview ==="
  if [[ -n "$ITEMS" ]]; then
    TITLE_XPATH=$(jq -r '.title // ""' "$SELECTORS_FILE")
    URL_XPATH=$(jq -r '.url // ""' "$SELECTORS_FILE")
    TITLE=$(evaluate_xpath "($ITEMS)[1]$TITLE_XPATH" 2>/dev/null || echo "(none)")
    URL=$(evaluate_xpath "($ITEMS)[1]$URL_XPATH" 2>/dev/null || echo "(none)")
    echo "  Title: $TITLE"
    echo "  URL:   $URL"
  fi
fi
