#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, validate } from '../scripts/lib/json-schema.mjs';
import { yamlParse as parseYaml } from '../scripts/lib/yaml.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
let passed = 0;
const failures = [];

function check(name, fn) {
  try {
    fn();
    passed++;
    console.log(`PASS ${name}`);
  } catch (error) {
    failures.push(`${name}: ${error.message}`);
    console.log(`FAIL ${name}: ${error.message}`);
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function loadJson(relative) {
  return JSON.parse(fs.readFileSync(path.join(root, relative), 'utf8'));
}

function schemaErrors(document, schema) {
  const errors = [];
  validate(document, schema, schema, '', errors);
  return errors;
}

const modelSchema = loadJson('registries/operating-model/workspace-scope-model.schema.json');
const recordSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const model = parseYaml(fs.readFileSync(path.join(root, 'standards/workspace/workspace-scope-model.yaml'), 'utf8'));

check('обе схемы используют поддерживаемое подмножество JSON Schema', () => {
  assertSupportedDeep(modelSchema, 'workspace-scope-model.schema.json');
  assertSupportedDeep(recordSchema, 'scoped-record.schema.json');
});

check('каноническая модель проходит собственную схему', () => {
  assert(schemaErrors(model, modelSchema).length === 0, schemaErrors(model, modelSchema)[0]);
});

check('пул содержит ровно шесть уникальных обязательных областей', () => {
  const expected = [
    'built-in-methodology',
    'organization-profile',
    'project-workspace',
    'repository-scope',
    'run-state',
    'user-profile',
  ];
  const actual = model.scope_types.map((scope) => scope.id).sort();
  assert(JSON.stringify(actual) === JSON.stringify(expected), `получен пул ${actual.join(', ')}`);
});

check('схема отклоняет неверный субъект канонической области', () => {
  const document = structuredClone(model);
  document.scope_types.find((scope) => scope.id === 'run-state').subject = 'repository';
  assert(schemaErrors(document, modelSchema).length > 0, 'несогласованная область прошла проверку');
});

const validRecord = {
  $schema: './scoped-record.schema.json',
  schema_version: 1,
  id: 'sample-rule',
  title: 'Пример нормы',
  record_type: 'norm',
  scope: { type: 'repository-scope', id: 'sample-repository', workspace_id: 'sample-project' },
  origin: { kind: 'declared', source_ref: 'owner-decision:sample-rule' },
  authority: { kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: 'owner-decision:sample-rule' },
  payload: {},
};

check('запись с явными областью, происхождением и полномочием допустима', () => {
  assert(schemaErrors(validRecord, recordSchema).length === 0, schemaErrors(validRecord, recordSchema)[0]);
});

check('область репозитория без рабочего пространства отклоняется', () => {
  const document = structuredClone(validRecord);
  delete document.scope.workspace_id;
  assert(schemaErrors(document, recordSchema).length > 0, 'дефект прошёл проверку');
});

check('рабочее пространство может явно ссылаться на профиль организации', () => {
  const document = structuredClone(validRecord);
  document.scope = {
    type: 'project-workspace',
    id: 'sample-project',
    organization_profile_id: 'sample-organization',
  };
  assert(schemaErrors(document, recordSchema).length === 0, schemaErrors(document, recordSchema)[0]);
});

check('ссылка на профиль организации запрещена в другой области', () => {
  const document = structuredClone(validRecord);
  document.scope.organization_profile_id = 'sample-organization';
  assert(schemaErrors(document, recordSchema).length > 0, 'связь областей прошла вне рабочего пространства');
});

check('встроенное происхождение не выдаёт внешний источник за встроенный', () => {
  const document = structuredClone(validRecord);
  document.origin = { kind: 'built-in', source_ref: 'project-file' };
  assert(schemaErrors(document, recordSchema).length > 0, 'дефект прошёл проверку');
});

check('невстроенное происхождение без источника отклоняется', () => {
  const document = structuredClone(validRecord);
  document.origin = { kind: 'imported' };
  assert(schemaErrors(document, recordSchema).length > 0, 'дефект прошёл проверку');
});

check('встроенное происхождение запрещено вне встроенной методологии', () => {
  const document = structuredClone(validRecord);
  document.origin = { kind: 'built-in' };
  document.authority = { kind: 'methodology-owner', authority_ref: 'methodology-owner' };
  assert(schemaErrors(document, recordSchema).length > 0, 'проектная запись выдала себя за встроенную');
});

check('встроенная запись требует полномочие владельца методологии', () => {
  const document = structuredClone(validRecord);
  document.scope = { type: 'built-in-methodology', id: 'built-in-methodology' };
  document.origin = { kind: 'built-in' };
  assert(schemaErrors(document, recordSchema).length > 0, 'проектное полномочие применилось к встроенной записи');
});

check('согласованная встроенная запись допустима', () => {
  const document = structuredClone(validRecord);
  document.scope = { type: 'built-in-methodology', id: 'built-in-methodology' };
  document.origin = { kind: 'built-in' };
  document.authority = { kind: 'methodology-owner', authority_ref: 'methodology-owner' };
  assert(schemaErrors(document, recordSchema).length === 0, schemaErrors(document, recordSchema)[0]);
});

check('физическое расположение не входит в конверт записи', () => {
  const document = structuredClone(validRecord);
  document.storage_path = '/temporary/location';
  assert(schemaErrors(document, recordSchema).length > 0, 'путь хранения стал частью идентичности записи');
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
