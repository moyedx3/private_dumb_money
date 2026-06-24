#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const SECRET_NAMES = new Set(["kDrop", "k_drop", "creatorUfvk", "creator_ufvk"]);
const SOURCE_EXTENSIONS = new Set([".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"]);

const rootDir = process.cwd();
const inputPaths = process.argv.slice(2);
const findings = [];

function collectSourceFiles(entries) {
  const files = [];
  for (const entry of entries) {
    const absolutePath = path.resolve(rootDir, entry);
    if (!fs.existsSync(absolutePath)) {
      findings.push({ file: entry, line: 1, column: 1, reason: "input path does not exist" });
      continue;
    }
    const stat = fs.statSync(absolutePath);
    if (stat.isDirectory()) {
      files.push(...collectSourceFiles(fs.readdirSync(absolutePath).map((name) => path.join(entry, name))));
      continue;
    }
    if (SOURCE_EXTENSIONS.has(path.extname(absolutePath))) {
      files.push(absolutePath);
    }
  }
  return files;
}

function sourceKindFor(file) {
  switch (path.extname(file)) {
    case ".tsx":
    case ".jsx":
      return ts.ScriptKind.TSX;
    case ".js":
    case ".mjs":
    case ".cjs":
      return ts.ScriptKind.JS;
    default:
      return ts.ScriptKind.TS;
  }
}

function propertyNameText(name) {
  if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
    return name.text;
  }
  return undefined;
}

function containsSecretReference(node) {
  let found = false;
  function visit(current) {
    if (found) {
      return;
    }
    if (ts.isIdentifier(current) && SECRET_NAMES.has(current.text)) {
      found = true;
      return;
    }
    if (ts.isPropertyAccessExpression(current) && SECRET_NAMES.has(current.name.text)) {
      found = true;
      return;
    }
    if (ts.isPropertyAssignment(current)) {
      const name = propertyNameText(current.name);
      if (name && SECRET_NAMES.has(name)) {
        found = true;
        return;
      }
    }
    ts.forEachChild(current, visit);
  }
  visit(node);
  return found;
}

function isConsoleCall(node) {
  return (
    ts.isCallExpression(node) &&
    ts.isPropertyAccessExpression(node.expression) &&
    ts.isIdentifier(node.expression.expression) &&
    node.expression.expression.text === "console"
  );
}

function isResultUseStateDeclaration(node) {
  if (!ts.isVariableDeclaration(node) || !ts.isArrayBindingPattern(node.name)) {
    return false;
  }
  const firstBinding = node.name.elements[0];
  return (
    firstBinding !== undefined &&
    ts.isBindingElement(firstBinding) &&
    ts.isIdentifier(firstBinding.name) &&
    firstBinding.name.text === "result" &&
    node.initializer !== undefined &&
    ts.isCallExpression(node.initializer) &&
    ts.isIdentifier(node.initializer.expression) &&
    node.initializer.expression.text === "useState"
  );
}

function hasForbiddenResultField(node) {
  let found = false;
  function visit(current) {
    if (found) {
      return;
    }
    if (ts.isPropertySignature(current) || ts.isPropertyDeclaration(current)) {
      const name = propertyNameText(current.name);
      if (name && SECRET_NAMES.has(name)) {
        found = true;
        return;
      }
    }
    ts.forEachChild(current, visit);
  }
  visit(node);
  return found;
}

function isSetResultCall(node) {
  return ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "setResult";
}

function hasForbiddenObjectField(node) {
  if (!ts.isObjectLiteralExpression(node)) {
    return false;
  }
  return node.properties.some((property) => {
    if (ts.isShorthandPropertyAssignment(property)) {
      return SECRET_NAMES.has(property.name.text);
    }
    if (ts.isPropertyAssignment(property) || ts.isMethodDeclaration(property)) {
      const name = propertyNameText(property.name);
      return name !== undefined && SECRET_NAMES.has(name);
    }
    return false;
  });
}

function isJsxTextBinding(node) {
  return ts.isJsxExpression(node) && node.expression !== undefined && !ts.isJsxAttribute(node.parent);
}

function location(sourceFile, node) {
  const position = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return {
    file: path.relative(rootDir, sourceFile.fileName),
    line: position.line + 1,
    column: position.character + 1
  };
}

function addFinding(sourceFile, node, reason) {
  findings.push({ ...location(sourceFile, node), reason });
}

function inspectSourceFile(sourceFile) {
  function visit(node) {
    if (isConsoleCall(node) && node.arguments.some((argument) => containsSecretReference(argument))) {
      addFinding(sourceFile, node, "console call includes kDrop or creatorUfvk");
    }

    if (isResultUseStateDeclaration(node)) {
      const typeArguments = node.initializer.typeArguments ?? [];
      if (typeArguments.some((typeArgument) => hasForbiddenResultField(typeArgument))) {
        addFinding(sourceFile, node, "result state type includes a secret field");
      }
    }

    if (isSetResultCall(node) && node.arguments.some((argument) => hasForbiddenObjectField(argument))) {
      addFinding(sourceFile, node, "result state update includes a secret field");
    }

    if (isJsxTextBinding(node) && containsSecretReference(node.expression)) {
      addFinding(sourceFile, node, "JSX text renders kDrop or creatorUfvk");
    }

    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
}

const files = collectSourceFiles(inputPaths.length > 0 ? inputPaths : ["src"]);
for (const file of files) {
  const source = fs.readFileSync(file, "utf8");
  const sourceFile = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, sourceKindFor(file));
  inspectSourceFile(sourceFile);
}

if (findings.length > 0) {
  for (const finding of findings) {
    console.error(`${finding.file}:${finding.line}:${finding.column} ${finding.reason}`);
  }
  process.exit(1);
}

console.log(`secret sink check passed (${files.length} files scanned)`);
