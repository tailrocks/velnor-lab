'use strict';
const fs = require('fs');

const input = (name) => process.env[`INPUT_${name.toUpperCase()}`] || '';
const env = (name) => process.env[name] || '';
const event = () => JSON.parse(fs.readFileSync(env('GITHUB_EVENT_PATH'), 'utf8'));

function canon(value) {
  if (Array.isArray(value)) return value.map(canon);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canon(value[key])]));
  }
  return value;
}

function normalize(text) {
  let out = text;
  for (const [from, to] of [[env('PROBE_TAG'), '<tag>'], [env('GITHUB_RUN_ID'), '<run_id>']]) {
    if (from) out = out.split(from).join(to);
  }
  return out.replace(/\b(refs\/pull\/)?\d+\/merge/g, (_, prefix) => `${prefix || ''}<n>/merge`);
}

function emit(probe, leg, data) {
  const body = JSON.parse(normalize(JSON.stringify(data)));
  const observation = canon({ probe, legs: { [leg]: body } });
  const text = `${JSON.stringify(observation, null, 2)}\n`;
  fs.mkdirSync('observation', { recursive: true });
  fs.writeFileSync(`observation/${probe}.json`, text);
  process.stdout.write(text);
}

function exportEnv(values) {
  const lines = Object.entries(values).map(([key, value]) => `${key}=${value}\n`);
  fs.appendFileSync(env('GITHUB_ENV'), lines.join(''));
}

function context() {
  const name = env('GITHUB_EVENT_NAME');
  const payload = event();
  const branch = name === 'pull_request' ? env('GITHUB_HEAD_REF') : env('GITHUB_REF_NAME');
  let tag = '';
  let role = '';
  if (name === 'schedule') {
    tag = `s${env('GITHUB_RUN_ID')}`;
    role = 'scheduled';
  } else if (name === 'workflow_dispatch') {
    tag = (payload.inputs && payload.inputs.tag) || `m${env('GITHUB_RUN_ID')}`;
    role = (payload.inputs && payload.inputs.role) || 'manual';
  } else {
    const found = /^probe-(\d+)-(.+)$/.exec(branch);
    if (!found) throw new Error(`branch ${branch} does not look like probe-<tag>-<role>`);
    [, tag, role] = found;
  }
  for (const value of [tag, role]) {
    if (!/^[A-Za-z0-9._-]+$/.test(value)) throw new Error('tag or role has characters that are not allowed');
  }
  let ordinal = 0;
  for (const dir of fs.existsSync('probes') ? fs.readdirSync('probes') : []) {
    const file = `probes/${dir}/runs/${branch}.txt`;
    if (fs.existsSync(file)) ordinal = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).length;
  }
  exportEnv({ PROBE_TAG: tag, PROBE_ROLE: role, PROBE_ORDINAL: ordinal, PROBE_EVENT: name });
}

function token() {
  const raw = env('ACTIONS_RUNTIME_TOKEN');
  let claims;
  try {
    claims = JSON.parse(Buffer.from(raw.split('.')[1], 'base64url').toString('utf8'));
  } catch (error) {
    throw new Error('the runtime token payload could not be decoded');
  }
  const names = Object.keys(claims).sort();
  let scopes = claims.ac;
  if (typeof scopes === 'string') scopes = JSON.parse(scopes);
  const data = {
    claims: Object.fromEntries(names.map((key) => [key, Array.isArray(claims[key]) ? 'array' : typeof claims[key]])),
    cache_scopes: (scopes || []).map((item) => ({ permission: item.Permission, scope: item.Scope })),
    scp: [...new Set(String(claims.scp || '').split(' ').filter(Boolean).map((item) => item.split(':')[0]))].sort(),
  };
  emit('token', env('GITHUB_EVENT_NAME'), data);
}

const FIELDS = {
  all: ['repository.full_name', 'repository.default_branch', 'repository.fork', 'repository.owner.login', 'sender.login', 'action'],
  push: ['ref', 'before', 'after', 'forced', 'created', 'deleted', 'base_ref', 'compare', 'commits', 'head_commit.id', 'pusher.name'],
  pull_request: [
    'number', 'pull_request.number', 'pull_request.title', 'pull_request.state', 'pull_request.draft', 'pull_request.merged',
    'pull_request.changed_files', 'pull_request.labels',
    'pull_request.user.login', 'pull_request.base.ref', 'pull_request.base.sha', 'pull_request.base.repo.full_name',
    'pull_request.head.ref', 'pull_request.head.sha', 'pull_request.head.repo.full_name', 'pull_request.head.repo.fork',
    'before', 'after',
  ],
  schedule: ['schedule'],
  workflow_dispatch: ['inputs', 'ref', 'workflow'],
};
const VARIABLES = [
  'GITHUB_EVENT_NAME', 'GITHUB_EVENT_PATH', 'GITHUB_REF', 'GITHUB_REF_NAME', 'GITHUB_REF_TYPE', 'GITHUB_BASE_REF',
  'GITHUB_HEAD_REF', 'GITHUB_SHA', 'GITHUB_REPOSITORY', 'GITHUB_REPOSITORY_OWNER', 'GITHUB_RUN_ATTEMPT',
  'GITHUB_API_URL', 'GITHUB_SERVER_URL', 'GITHUB_WORKFLOW_REF', 'GITHUB_WORKSPACE',
];
const VALUES = ['GITHUB_EVENT_NAME', 'GITHUB_REF', 'GITHUB_REF_TYPE', 'GITHUB_BASE_REF', 'GITHUB_HEAD_REF', 'GITHUB_REF_NAME'];

function shape(value) {
  if (value === undefined) return 'absent';
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  if (typeof value === 'string') return /^[0-9a-f]{40}$/.test(value) ? 'sha' : 'string';
  return typeof value;
}

function eventPayload() {
  const name = env('GITHUB_EVENT_NAME');
  const payload = event();
  const fields = {};
  for (const path of [...FIELDS.all, ...(FIELDS[name] || [])]) {
    fields[path] = shape(path.split('.').reduce((node, key) => (node === undefined || node === null ? undefined : node[key]), payload));
  }
  const variables = Object.fromEntries(VARIABLES.map((key) => [key, process.env[key] === undefined ? 'absent' : process.env[key] === '' ? 'empty' : 'set']));
  const values = Object.fromEntries(VALUES.map((key) => [key, env(key)]));
  emit('event-payload', name, { fields, top_level: Object.keys(payload).sort(), values, variables });
}

async function jobLog(job) {
  const headers = { Authorization: `Bearer ${input('github-token')}`, 'User-Agent': 'velnor-lab', Accept: 'application/vnd.github+json' };
  const base = `https://api.github.com/repos/${env('GITHUB_REPOSITORY')}/actions`;
  const jobs = await (await fetch(`${base}/runs/${env('GITHUB_RUN_ID')}/jobs?per_page=100`, { headers })).json();
  const found = jobs.jobs.find((item) => item.name === job);
  if (!found) throw new Error(`job ${job} not found`);
  const response = await fetch(`${base}/jobs/${found.id}/logs`, { headers });
  if (!response.ok) throw new Error(`log of ${job}: HTTP ${response.status}`);
  return (await response.text()).split('\n').map((line) => line.replace(/^\S+Z /, '').replace(/\u001b\[[0-9;]*m/g, '').replace(/\r$/, ''));
}

function stepOutputs(lines, marker) {
  const blocks = [];
  for (let at = 0; at < lines.length; at += 1) {
    if (!(lines[at].startsWith('##[group]Run ') && lines[at].includes(marker))) continue;
    while (at < lines.length && lines[at] !== '##[endgroup]') at += 1;
    const block = [];
    for (at += 1; at < lines.length && !lines[at].startsWith('##[group]') && lines[at] !== ''; at += 1) {
      if (!lines[at].startsWith('[command]') && !lines[at].startsWith('Cache Size:')) block.push(lines[at]);
    }
    blocks.push(block);
    at -= 1;
  }
  return blocks;
}

async function record() {
  const data = JSON.parse(input('data'));
  if (input('log-job')) data.step_output = stepOutputs(await jobLog(input('log-job')), input('log-steps'));
  emit(input('probe'), input('leg'), data);
}

function compare() {
  const probe = input('probe');
  const observed = JSON.parse(fs.readFileSync(`observation/${probe}.json`, 'utf8'));
  const stored = JSON.parse(fs.readFileSync(`conformance/${probe}.json`, 'utf8'));
  let failed = false;
  for (const [leg, value] of Object.entries(observed.legs)) {
    const want = JSON.stringify(canon(stored.legs[leg]));
    const got = JSON.stringify(canon(value));
    if (want !== got) {
      failed = true;
      process.stdout.write(`::error title=probe ${probe} differs::probe ${probe}, leg ${leg} differs from conformance/${probe}.json\n`);
      process.stdout.write(`stored:   ${want}\nobserved: ${got}\n`);
    }
  }
  if (failed) process.exit(1);
  process.stdout.write(`probe ${probe} matches conformance/${probe}.json\n`);
}

async function main() {
  const mode = input('mode');
  if (mode === 'context') context();
  else if (mode === 'token') token();
  else if (mode === 'event') eventPayload();
  else if (mode === 'record') await record();
  else if (mode === 'compare') compare();
  else throw new Error(`unknown mode ${mode}`);
}

main().catch((error) => {
  process.stdout.write(`::error::${error.message}\n`);
  process.exit(1);
});
