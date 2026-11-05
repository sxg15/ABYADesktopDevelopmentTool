#!/usr/bin/env node
// Offline candidate lookup. Source URLs and query text are data, never commands.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const ENUMS = Object.freeze({
  uiKinds: ['menu', 'hud', 'inventory', 'dialogue', 'settings', 'map', 'results', 'level-select'],
  inputs: ['pointer', 'keyboard', 'gamepad', 'touch'],
  density: ['low', 'medium', 'high'],
  scope: ['full', 'partial', 'material-only'],
  categories: ['layout', 'style', 'state-feedback'],
});
const REQUIRED = ['uiKinds', 'inputs', 'density', 'scope', 'brandLocked'];
const ALLOWED = [...REQUIRED, 'categories', 'query', 'templateExplicit', 'preferredSourceUrls'];
const BASE_NOTES = [
  'Results are candidates, not an adopted art template, planner approval, or runtime evidence.',
  'Review the source and current task references before recording borrow/adapt/avoid decisions.',
  'Input tags mean adaptation scope; they do not claim tested input support.',
];
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const own = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
const parseJson = text => JSON.parse(text.replace(/^\uFEFF/u, ''));

export function loadLibrary() {
  const library = parseJson(readFileSync(new URL('../assets/ui-reference-library.json', import.meta.url), 'utf8'));
  validateLibrary(library);
  return library;
}

function validateLibrary(library) {
  if (!object(library) || library.schemaVersion !== 1 || !Array.isArray(library.entries)) {
    throw new TypeError('Invalid candidate library schema.');
  }
  const ids = new Set();
  for (const entry of library.entries) {
    if (!object(entry) || typeof entry.id !== 'string' || ids.has(entry.id)
        || !ENUMS.categories.includes(entry.category) || !object(entry.source)
        || entry.humanApproved !== false || entry.runtimeVerified !== false) {
      throw new TypeError('Library entries must be unique unapproved, unverified candidates.');
    }
    ids.add(entry.id);
    for (const key of ['uiKinds', 'inputs', 'densities', 'borrow', 'adapt', 'avoid', 'limitations']) {
      if (!Array.isArray(entry[key]) || !entry[key].length || entry[key].some(v => typeof v !== 'string')) {
        throw new TypeError(`Invalid candidate field: ${entry.id}.${key}`);
      }
    }
  }
}

function validateRequest(request) {
  const errors = [], missingContext = [];
  if (!object(request)) return { errors: ['Request must be a JSON object.'], missingContext };
  for (const key of Object.keys(request)) if (!ALLOWED.includes(key)) errors.push(`Unknown field: ${key}`);
  for (const key of REQUIRED) if (!own(request, key)) missingContext.push(key);
  for (const key of ['uiKinds', 'inputs', 'categories']) {
    if (!own(request, key)) continue;
    const value = request[key];
    if (!Array.isArray(value) || !value.length || value.some(v => !ENUMS[key].includes(v))) {
      errors.push(`${key} must be a nonempty array of: ${ENUMS[key].join(', ')}.`);
    } else if (new Set(value).size !== value.length) errors.push(`${key} must not contain duplicates.`);
  }
  for (const key of ['density', 'scope']) {
    if (own(request, key) && !ENUMS[key].includes(request[key])) {
      errors.push(`${key} must be one of: ${ENUMS[key].join(', ')}.`);
    }
  }
  for (const key of ['brandLocked', 'templateExplicit']) {
    if (own(request, key) && typeof request[key] !== 'boolean') errors.push(`${key} must be boolean.`);
  }
  if (own(request, 'query') && (typeof request.query !== 'string' || request.query.length > 200)) {
    errors.push('query must be a literal string of at most 200 characters.');
  }
  if (own(request, 'preferredSourceUrls')) {
    const urls = request.preferredSourceUrls;
    if (!Array.isArray(urls) || urls.length > 8 || urls.some(url => !isSourceUrl(url))) {
      errors.push('preferredSourceUrls must be an array of at most 8 HTTP(S) URL strings.');
    }
  }
  return { errors, missingContext };
}

function isSourceUrl(value) {
  if (typeof value !== 'string' || value.length > 2048) return false;
  try { return ['https:', 'http:'].includes(new URL(value).protocol); } catch { return false; }
}

export function queryUiReferences(request, library, { limit = 6 } = {}) {
  const validation = validateRequest(request);
  if (!Number.isInteger(limit) || limit < 1 || limit > 20) validation.errors.push('limit must be an integer from 1 to 20.');
  const result = {
    schemaVersion: 1, status: 'ok', ...validation, candidates: [], excluded: [], notes: [...BASE_NOTES],
  };
  if (validation.errors.length) return { ...result, status: 'invalid-request' };
  if (validation.missingContext.length) return { ...result, status: 'requires-context' };
  validateLibrary(library);
  result.notes.push(request.templateExplicit === true
    ? 'An explicit template request is context only; lookup still does not select or adopt a template.'
    : 'No explicit template request: style entries expose only limited material borrow candidates.');
  if (request.brandLocked) result.notes.push('Existing brand colors, fonts, semantic colors, and approved structure remain binding.');
  const ranked = [];
  for (const entry of library.entries) {
    const matchedKinds = request.uiKinds.filter(kind => entry.uiKinds.includes(kind));
    const rejected = [];
    const exactDensity = entry.densities.includes(request.density);
    const styleFragment = entry.category === 'style' && request.scope !== 'full';
    if (!matchedKinds.length) rejected.push('No matching uiKinds.');
    if (request.inputs.some(input => !entry.inputs.includes(input))) rejected.push('Not all requested inputs are in adaptation scope.');
    if (request.categories && !request.categories.includes(entry.category)) rejected.push('Outside requested categories.');
    if (request.scope === 'material-only' && entry.category !== 'style') rejected.push('Material-only scope excludes layout and state-pattern candidates.');
    if (!exactDensity && !styleFragment) rejected.push('Density is outside this candidate scope.');
    if (entry.category === 'style' && request.scope === 'full'
        && request.uiKinds.some(kind => !(entry.fullUiKinds ?? entry.uiKinds).includes(kind))) {
      rejected.push('Full-screen style is outside the requested UI kind; consider a separate partial material request.');
    }
    if (rejected.length) { result.excluded.push({ id: entry.id, reasons: rejected }); continue; }
    const matchReasons = [`UI kind: ${matchedKinds.join(', ')}.`, `Inputs for adaptation: ${request.inputs.join(', ')}.`];
    const mismatchReasons = [];
    if (exactDensity) matchReasons.push(`Density: ${request.density}.`);
    else mismatchReasons.push('Source density does not fit the whole screen; only a bounded material fragment is offered.');
    const unmatchedKinds = request.uiKinds.filter(kind => !entry.uiKinds.includes(kind));
    if (unmatchedKinds.length) mismatchReasons.push(`Does not address: ${unmatchedKinds.join(', ')}.`);
    const urls = [entry.source.url, entry.source.previewUrl, entry.source.repositoryUrl];
    const preferred = (request.preferredSourceUrls ?? []).some(url => urls.includes(url));
    if (preferred) matchReasons.push('Matches a supplied source URL; task-fit restrictions still apply.');
    const query = (request.query ?? '').trim().toLowerCase();
    const text = [entry.id, entry.title, ...(entry.tags ?? [])].join(' ').toLowerCase();
    const queryHit = Boolean(query && text.includes(query));
    if (queryHit) matchReasons.push('Contains the supplied literal query in its title or tags.');
    else if (query) mismatchReasons.push('Literal query does not match this candidate title or tags.');
    let allowedBorrow = entry.borrow;
    if (request.scope === 'material-only') allowedBorrow = entry.materialBorrow;
    else if (request.scope === 'partial' && entry.category !== 'style') allowedBorrow = entry.partialBorrow;
    else if (request.brandLocked) allowedBorrow = entry.brandNeutralBorrow;
    else if (entry.category === 'style' && (styleFragment || request.templateExplicit !== true)) allowedBorrow = entry.materialBorrow;
    if (!allowedBorrow?.length) { result.excluded.push({ id: entry.id, reasons: ['No borrow within the requested protected scope.'] }); continue; }
    if (entry.category === 'style' && (styleFragment || request.templateExplicit !== true)) {
      mismatchReasons.push('Limited to material exploration; this result does not justify a full style replacement.');
    }
    if (request.brandLocked) mismatchReasons.push('Borrow is restricted by the existing brand; no palette or font replacement.');
    const candidate = {
      id: entry.id, title: entry.title, category: entry.category, source: { ...entry.source },
      matchReasons, mismatchReasons, allowedBorrow: [...allowedBorrow], adapt: [...entry.adapt],
      avoid: [...entry.avoid], limitations: [...entry.limitations],
      scopeConstraints: { preserveBrand: request.brandLocked, preserveLayout: request.scope !== 'full', materialOnly: request.scope === 'material-only' },
      humanApproved: false, runtimeVerified: false,
    };
    ranked.push({ candidate, preferred, kinds: matchedKinds.length, exactDensity, queryHit });
  }
  ranked.sort((a, b) => Number(b.preferred) - Number(a.preferred) || b.kinds - a.kinds
    || Number(b.exactDensity) - Number(a.exactDensity) || Number(b.queryHit) - Number(a.queryHit)
    || (a.candidate.id < b.candidate.id ? -1 : a.candidate.id > b.candidate.id ? 1 : 0));
  result.candidates = ranked.slice(0, limit).map(item => item.candidate);
  if (!result.candidates.length) result.status = 'no-matches';
  return result;
}

function runCli(args) {
  let inputPath, limit = 6;
  const seen = new Set();
  for (let i = 0; i < args.length; i++) {
    const flag = args[i];
    if (flag === '--help' && args.length === 1) {
      console.log('Usage: node query-ui-references.mjs [--input <request.json|->] [--limit <1..20>]\nWithout --input, read one JSON object from stdin. Writes JSON to stdout only.');
      return;
    }
    if (!['--input', '--limit'].includes(flag) || seen.has(flag) || i + 1 >= args.length) {
      throw new TypeError('Use --input <request.json|-> and/or --limit <1..20>, each at most once.');
    }
    seen.add(flag);
    const value = args[++i];
    if (flag === '--input') {
      if (value.startsWith('--')) throw new TypeError('--input requires a file path or -.');
      inputPath = value;
    } else {
      if (!/^\d+$/u.test(value)) throw new TypeError('--limit must be an integer from 1 to 20.');
      limit = Number(value);
    }
  }
  if (!Number.isInteger(limit) || limit < 1 || limit > 20) throw new TypeError('--limit must be an integer from 1 to 20.');
  const text = readFileSync(inputPath && inputPath !== '-' ? inputPath : 0, 'utf8');
  const result = queryUiReferences(parseJson(text), loadLibrary(), { limit });
  console.log(JSON.stringify(result, null, 2));
  if (result.status === 'invalid-request') process.exitCode = 2;
}

const isMain = process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (isMain) {
  try { runCli(process.argv.slice(2)); }
  catch (error) {
    console.log(JSON.stringify({
      schemaVersion: 1, status: 'invalid-request', missingContext: [],
      errors: [error instanceof Error ? error.message : 'Invalid input.'], candidates: [], excluded: [], notes: [...BASE_NOTES],
    }, null, 2));
    process.exitCode = 2;
  }
}
