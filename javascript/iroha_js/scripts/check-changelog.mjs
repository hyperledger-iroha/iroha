#!/usr/bin/env node

/**
 * Ensures CHANGELOG.md contains an entry for the current package version and
 * that the version increases monotonically compared to previous releases.
 *
 * Run via `npm run check:changelog` or automatically through `prepublishOnly`.
 */

import fs from "node:fs";
import process from "node:process";

/**
 * Resolve a possibly overridden path (via env var) to a file URL.
 * Allows injecting alternative fixtures during tests while keeping the default
 * repository-relative resolution for normal CLI usage.
 */
function resolvePath(envVar, fallback) {
  if (!envVar) {
    return new URL(fallback, import.meta.url);
  }
  if (envVar.startsWith("file://")) {
    return new URL(envVar);
  }
  return new URL(envVar, import.meta.url);
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseSemver(version) {
  const match =
    /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(
      version,
    );
  if (!match) {
    fail(
      `Version '${version}' is not a valid semver (expected <major>.<minor>.<patch> with optional -prerelease/+build).`,
    );
  }
  const [, major, minor, patch, prerelease = ""] = match;
  return {
    major: Number(major),
    minor: Number(minor),
    patch: Number(patch),
    prerelease,
  };
}

function compareIdentifiers(a, b) {
  const numericA = Number(a);
  const numericB = Number(b);
  const aIsNumeric = !Number.isNaN(numericA);
  const bIsNumeric = !Number.isNaN(numericB);
  if (aIsNumeric && bIsNumeric) {
    return Math.sign(numericA - numericB);
  }
  if (aIsNumeric) {
    return -1;
  }
  if (bIsNumeric) {
    return 1;
  }
  if (a < b) {
    return -1;
  }
  if (a > b) {
    return 1;
  }
  return 0;
}

function comparePrerelease(a, b) {
  if (!a && !b) {
    return 0;
  }
  if (!a) {
    return 1;
  }
  if (!b) {
    return -1;
  }
  const segmentsA = a.split(".");
  const segmentsB = b.split(".");
  const length = Math.max(segmentsA.length, segmentsB.length);
  for (let i = 0; i < length; i += 1) {
    const segA = segmentsA[i];
    const segB = segmentsB[i];
    if (segA === undefined) {
      return -1;
    }
    if (segB === undefined) {
      return 1;
    }
    const cmp = compareIdentifiers(segA, segB);
    if (cmp !== 0) {
      return cmp;
    }
  }
  return 0;
}

function isSemverGreater(a, b) {
  const va = parseSemver(a);
  const vb = parseSemver(b);
  if (va.major !== vb.major) {
    return va.major > vb.major;
  }
  if (va.minor !== vb.minor) {
    return va.minor > vb.minor;
  }
  if (va.patch !== vb.patch) {
    return va.patch > vb.patch;
  }
  return comparePrerelease(va.prerelease, vb.prerelease) > 0;
}

const pkgPath = resolvePath(
  process.env.CHECK_CHANGELOG_PACKAGE_JSON ?? "",
  "../package.json",
);
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
const version = pkg.version;

if (!version) {
  fail("Unable to read version from package.json");
}

const changelogPath = resolvePath(
  process.env.CHECK_CHANGELOG_CHANGELOG ?? "",
  "../CHANGELOG.md",
);
const changelog = fs.readFileSync(changelogPath, "utf8");
const escapedVersion = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const headingPattern = new RegExp(`^##\\s+\\[?${escapedVersion}\\]?`, "m");

if (!headingPattern.test(changelog)) {
  fail(
    `CHANGELOG.md is missing an entry for version ${version}. ` +
      "Add a heading such as `## [" +
      version +
      "] - YYYY-MM-DD` before publishing.",
  );
}

const versionHeadings = Array.from(
  changelog.matchAll(/^##\s+\[?([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)\]?/gm),
).map((match) => match[1]);

if (versionHeadings.length === 0) {
  fail("CHANGELOG.md does not contain any release headings (## <version>).");
}

if (versionHeadings[0] !== version) {
  fail(
    `CHANGELOG.md entry for ${version} is not the most recent release. ` +
      "Ensure the new version appears at the top of the changelog.",
  );
}

if (versionHeadings.slice(1).includes(version)) {
  fail(
    `Version ${version} appears multiple times in CHANGELOG.md. ` +
      "Remove duplicate headings before publishing.",
  );
}

if (versionHeadings.length > 1) {
  const previous = versionHeadings[1];
  if (!isSemverGreater(version, previous)) {
    fail(
      `Version ${version} must be greater than the previous release ${previous}.`,
    );
  }
}

const summary =
  versionHeadings.length > 1
    ? `CHANGELOG.md entry for ${version} is present and newer than ${versionHeadings[1]}.`
    : `CHANGELOG.md entry for ${version} is present (no previous release detected).`;
console.log(summary);
