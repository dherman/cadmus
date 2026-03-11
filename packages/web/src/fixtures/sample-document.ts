/**
 * Sample document exercising every node and mark type in the Cadmus schema.
 * Used for visual QA during development.
 */
export const sampleDocument = `
<h1>Welcome to Cadmus</h1>
<p>This document exercises every node and mark type in the schema. Use it for visual QA during development.</p>

<h2>Inline Marks</h2>
<p>
  Here is <strong>bold text</strong>, <em>italic text</em>,
  <s>strikethrough text</s>, and <code>inline code</code>.
  You can also have <strong><em>bold italic</em></strong> combined.
  Links look like <a href="https://example.com">this example link</a>.
</p>

<h2>Headings</h2>
<h1>Heading 1</h1>
<h2>Heading 2</h2>
<h3>Heading 3</h3>
<h4>Heading 4</h4>
<h5>Heading 5</h5>
<h6>Heading 6</h6>

<h2>Lists</h2>
<h3>Bullet List</h3>
<ul>
  <li>First item</li>
  <li>Second item with <strong>bold</strong></li>
  <li>
    Nested list:
    <ul>
      <li>Nested item one</li>
      <li>Nested item two</li>
    </ul>
  </li>
  <li>Back to top level</li>
</ul>

<h3>Ordered List</h3>
<ol>
  <li>Step one</li>
  <li>Step two with <em>emphasis</em></li>
  <li>
    Step three with nested:
    <ol>
      <li>Sub-step A</li>
      <li>Sub-step B</li>
    </ol>
  </li>
</ol>

<h2>Code</h2>
<h3>Inline Code</h3>
<p>Use <code>const x = 42;</code> for inline snippets.</p>

<h3>Code Block</h3>
<pre><code class="language-typescript">function greet(name: string): string {
  return \`Hello, \${name}!\`;
}

console.log(greet('Cadmus'));
</code></pre>

<pre><code class="language-python">def greet(name: str) -> str:
    return f"Hello, {name}!"

print(greet("Cadmus"))
</code></pre>

<h2>Blockquotes</h2>
<blockquote>
  <p>A simple blockquote with a single paragraph.</p>
</blockquote>

<blockquote>
  <p>A blockquote with multiple paragraphs.</p>
  <p>This is the second paragraph inside the blockquote.</p>
  <blockquote>
    <p>And a nested blockquote within it.</p>
  </blockquote>
</blockquote>

<h2>Horizontal Rule</h2>
<p>Above the rule.</p>
<hr>
<p>Below the rule.</p>

<h2>Hard Break</h2>
<p>This line ends with a hard break.<br>And this is the next line in the same paragraph.</p>

<h2>Image</h2>
<img src="https://via.placeholder.com/640x360" alt="Placeholder image">

<h2>Paragraph</h2>
<p>A final paragraph to end the document. The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.</p>
`;
