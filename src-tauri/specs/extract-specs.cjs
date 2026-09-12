// Extractor unificado: genera src-tauri/specs/actions.json desde server.ts
// - prompts con ${...} -> {{...}} (placeholders que rellena Rust)
// - schemas Gemini: Type.X -> "X" via proxy
// - fallbacks literales evaluados con el mismo proxy
const fs = require('fs');
const path = require('path');

const SRC = 'C:\\Users\\tomas\\Desktop\\NodeFlow\\nodeflow-desktop\\server.ts';
const OUT = 'C:\\Users\\tomas\\Desktop\\NodeFlow\\nodeflow-desktop\\src-tauri\\specs\\actions.json';
const src = fs.readFileSync(SRC, 'utf8');
const start = src.indexOf('app.post("/api/ai/action"');
const end = src.indexOf('async function startServer');
const body = src.slice(start, end);

const Type = new Proxy({}, { get: (_, k) => k });
const toPlaceholders = (s) => s.replace(/\$\{([^}]+)\}/g, (_, e) => '{{' + e.trim().replace(/\s+/g, ' ') + '}}');

// ramas (soporta condiciones compuestas: type === "a" || type === "b")
const branchRe = /(?:if|else\s+if)\s*\(\s*((?:type\s*===\s*"[a-z_]+"\s*(?:\|\|\s*)?)+)\)/g;
const marks = [];
let m;
while ((m = branchRe.exec(body))) {
  const names = [...m[1].matchAll(/type\s*===\s*"([a-z_]+)"/g)].map(x => x[1]);
  if (names.length) marks.push({ names, pos: m.index });
}

function matchBalanced(s, i, open = '{') {
  const close = { '{': '}', '[': ']', '(': ')' }[open];
  let depth = 0, j = i, instr = null;
  while (j < s.length) {
    const c = s[j];
    if (instr) { if (c === '\\') { j += 2; continue; } if (c === instr) instr = null; }
    else if (c === '"' || c === "'" || c === '`') instr = c;
    else if (c === open) depth++;
    else if (c === close) { depth--; if (depth === 0) return j; }
    j++;
  }
  return -1;
}

function grabTemplate(s, name) {
  const re = new RegExp('const\\s+' + name + '\\s*=\\s*`');
  const mm = re.exec(s);
  if (!mm) return null;
  const i = s.indexOf('`', mm.index);
  let j = i + 1;
  while (j < s.length) { if (s[j] === '\\') { j += 2; continue; } if (s[j] === '`') return s.slice(i + 1, j); j++; }
  return null;
}

function grabLiterals(s, re) {
  const out = {};
  let mm;
  while ((mm = re.exec(s))) {
    const name = mm[1];
    // saltar espacios hasta el literal
    let p = mm.index + mm[0].length;
    while (p < s.length && /\s/.test(s[p])) p++;
    const open = s[p];
    if (open !== '{' && open !== '[') continue;
    const closeIdx = matchBalanced(s, p, open);
    if (closeIdx < 0) continue;
    let literal = s.slice(p, closeIdx + 1);
    literal = toPlaceholders(literal);
    try {
      out[name] = JSON.parse(JSON.stringify(eval('(' + literal + ')')));
    } catch (e) {
      out[name] = { __error: String(e).slice(0, 90) };
    }
  }
  return out;
}

// clave de respuesta por accion + campos extra
const RESP = {
  branch: { key: 'variations' }, explore: { key: 'variations' }, socratic: { key: 'variations' },
  critique: { key: 'variations' }, devils_advocate: { key: 'variations' },
  hybrid: { key: 'hybrid' }, synthesize: { key: 'synthesis' },
  find_bridges: { key: 'bridges', nested: 'bridges' }, braindump: { key: 'structure' },
  refresh_templates: { key: 'templates', extra: { source: 'gemini' } },
};

const specs = {};
for (let i = 0; i < marks.length; i++) {
  const name = marks[i].names[0];
  const aliases = marks[i].names.slice(1);
  const blk = body.slice(marks[i].pos, i + 1 < marks.length ? marks[i + 1].pos : body.length);

  const prompt = grabTemplate(blk, 'prompt') || grabTemplate(blk, 'promptText');
  const schemas = grabLiterals(blk, /const\s+(\w*[Ss]chema\w*)\s*=/g);
  const fallbacks = grabLiterals(blk, /const\s+(\w*[Ff]allback\w*)\s*=/g);

  // sub-ramas dentro de la rama (ej. synthesize con critique)
  const subConds = [...blk.matchAll(/(?:const|let)\s+(\w+)\s*=\s*req\.body\.(\w+)/g)].map(x => `${x[2]}`);
  const innerIfs = [...blk.matchAll(/if\s*\(\s*([^)]{0,60}?)\s*\)\s*\{/g)].map(x => x[1].slice(0, 50)).slice(0, 4);

  specs[name] = {
    aliases,
    prompt: prompt ? toPlaceholders(prompt) : null,
    schema: schemas[Object.keys(schemas)[0]] || null,
    schema_alt: Object.keys(schemas)[1] ? schemas[Object.keys(schemas)[1]] : null,
    prompt_alt: grabTemplate(blk, 'critiquePrompt') ? toPlaceholders(grabTemplate(blk, 'critiquePrompt')) : null,
    schemas_all: Object.keys(schemas),
    fallbacks,
    body_fields: subConds,
    inner_conditions: innerIfs,
    response: RESP[name] || { key: 'unknown' },
  };
}

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, JSON.stringify(specs, null, 1), 'utf8');
console.log('escrito:', OUT, '(' + (fs.statSync(OUT).size / 1024).toFixed(1) + ' KB)\n');
for (const [n, s] of Object.entries(specs)) {
  const errs = Object.entries(s.fallbacks).filter(([, v]) => v && v.__error).map(([k]) => k);
  console.log(`${n.padEnd(18)} prompt:${s.prompt ? s.prompt.length + 'ch' : 'NO'} schema:${s.schema && !s.schema.__error ? 'ok' : 'ERR'} alt:${s.schema_alt ? 'si' : '-'} fallbacks:[${Object.keys(s.fallbacks).join(',')}]${errs.length ? ' ERR:' + errs : ''} response_key:${s.response.key} inner:${JSON.stringify(s.inner_conditions)}`);
}
