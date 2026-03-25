import express from 'express';
import { SCHEMA_VERSION } from '@cadmus/doc-schema';
import { serialize } from './serialize';
import { parse } from './parse';
import { diff } from './diff';
import { merge } from './merge';

const app = express();
app.use(express.json({ limit: '10mb' }));

const PORT = process.env.SIDECAR_PORT || 3001;

app.get('/health', (_req, res) => {
  res.json({ ok: true, schema_version: SCHEMA_VERSION });
});

app.post('/serialize', async (req, res) => {
  try {
    const { doc, schema_version } = req.body;

    if (schema_version !== undefined && schema_version !== SCHEMA_VERSION) {
      res.status(400).json({
        error: `Schema version mismatch: expected ${SCHEMA_VERSION}, got ${schema_version}`,
      });
      return;
    }

    const markdown = serialize(doc);
    res.json({ markdown });
  } catch (err) {
    console.error('Serialize error:', err);
    res.status(500).json({ error: 'Serialization failed' });
  }
});

app.post('/parse', async (req, res) => {
  try {
    const { markdown, schema_version } = req.body;

    if (schema_version !== undefined && schema_version !== SCHEMA_VERSION) {
      res.status(400).json({
        error: `Schema version mismatch: expected ${SCHEMA_VERSION}, got ${schema_version}`,
      });
      return;
    }

    const doc = parse(markdown);
    res.json({ doc });
  } catch (err) {
    console.error('Parse error:', err);
    res.status(500).json({ error: 'Parsing failed' });
  }
});

app.post('/diff', async (req, res) => {
  try {
    const { old_doc, new_doc } = req.body;
    const steps = diff(old_doc, new_doc);
    res.json({ steps });
  } catch (err) {
    console.error('Diff error:', err);
    res.status(500).json({ error: 'Diff computation failed' });
  }
});

app.post('/merge', async (req, res) => {
  try {
    const { base_doc, current_doc, new_doc } = req.body;
    const steps = merge(base_doc, current_doc, new_doc);
    res.json({ steps });
  } catch (err) {
    console.error('Merge error:', err);
    res.status(500).json({ error: 'Merge computation failed' });
  }
});

app.listen(PORT, () => {
  console.log(`Sidecar listening on port ${PORT} (schema v${SCHEMA_VERSION})`);
});
