// Emits deploy/seeds/cavp_packs.json: the fixed CAVP question packs (prompt.json) plus the
// answer key the API grades against.
//
// The generator deliberately uses the same @noble/post-quantum build that the browser's
// step 03 (本地運算) runs, so "the reference answer" and "what a correct student gets" are
// the same computation. Everything is derived from fixed seeds, so re-running this script
// reproduces byte-identical packs.
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { ml_dsa65 } from "@noble/post-quantum/ml-dsa.js";

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(here, "../deploy/seeds/cavp_packs.json");

const PARAMETER_SET = "ML-DSA-65";
const hex = (bytes) => Buffer.from(bytes).toString("hex").toUpperCase();
const unhex = (s) => new Uint8Array(Buffer.from(s, "hex"));
/** Deterministic filler so packs are reproducible without a PRNG. */
const derive = (label, len) => {
  const out = Buffer.alloc(len);
  let offset = 0;
  let counter = 0;
  while (offset < len) {
    const block = createHash("sha256").update(`${label}:${counter++}`).digest();
    block.copy(out, offset);
    offset += block.length;
  }
  return new Uint8Array(out);
};

// The first two seeds are the ones the dev system shipped (see the prompt.json in the
// screenshots); the third keeps the pack at the documented three test cases.
const SEEDS = [
  "B512EEEEB054DF62F61935C22F24465FBF982EA8B4FFA913EEBFABD92B1F4D64",
  "B850D898A3D3D11C4E64ADE5A86FFED951B237C60D2A67A2DEF0A792B8F6990D",
  hex(derive("mldsa65-02/keygen/seed/3", 32)),
];

const MESSAGES = ["4207B9170C", hex(derive("mldsa65-02/siggen/msg/2", 48)), hex(derive("mldsa65-02/siggen/msg/3", 72))];
const CONTEXTS = ["1C", "", hex(derive("mldsa65-02/siggen/ctx/3", 8))];

const keypairs = SEEDS.map((seed) => ml_dsa65.keygen(unhex(seed)));

const registration = {
  algorithm: "ML-DSA",
  revision: "FIPS204",
  parameterSets: ["ML-DSA-44", "ML-DSA-65", "ML-DSA-87"],
  selectedParameterSet: PARAMETER_SET,
  vectorTypes: ["AFT", "VAL"],
  selectedVectorType: "AFT",
  sample: true,
  capabilityVsId: 42,
};

const signatureOptions = {
  signatureInterfaces: ["external", "internal"],
  selectedSignatureInterface: "external",
  preHash: ["pure", "preHash"],
  selectedPreHash: "pure",
  externalMu: false,
  deterministic: true,
  capabilityLimits: {
    messageLength: { min: 8, max: 65536, increment: 8, unit: "bits" },
    contextLength: { min: 0, max: 2040, increment: 8, unit: "bits" },
    hashAlgorithms: [
      "SHA2-224", "SHA2-256", "SHA2-384", "SHA2-512", "SHA2-512/224", "SHA2-512/256",
      "SHA3-224", "SHA3-256", "SHA3-384", "SHA3-512",
    ],
  },
};

const prompt = (vsId, mode, tests) => ({
  vsId,
  algorithm: "ML-DSA",
  mode,
  revision: "FIPS204",
  isSample: true,
  testGroups: [{ tgId: 1, testType: "AFT", parameterSet: PARAMETER_SET, tests }],
});

// --- keyGen -----------------------------------------------------------------
const keyGenPack = {
  slug: "mldsa65-02-keygen",
  capability: "key_gen",
  parameterSet: PARAMETER_SET,
  vsId: 91021,
  registration: { ...registration, mode: "keyGen" },
  prompt: prompt(91021, "keyGen", SEEDS.map((seed, i) => ({ tcId: i + 1, seed }))),
  answerKey: {
    vsId: 91021,
    mode: "keyGen",
    parameterSet: PARAMETER_SET,
    cases: keypairs.map((kp, i) => ({
      tcId: i + 1,
      pk: hex(kp.publicKey),
      sk: hex(kp.secretKey),
    })),
  },
};

// --- sigGen -----------------------------------------------------------------
const sigGenTests = keypairs.map((kp, i) => ({
  tcId: i + 1,
  sk: hex(kp.secretKey),
  message: MESSAGES[i],
  context: CONTEXTS[i],
}));

const sigGenPack = {
  slug: "mldsa65-02-siggen",
  capability: "sig_gen",
  parameterSet: PARAMETER_SET,
  vsId: 91022,
  registration: { ...registration, mode: "sigGen", ...signatureOptions },
  prompt: prompt(91022, "sigGen", sigGenTests),
  answerKey: {
    vsId: 91022,
    mode: "sigGen",
    parameterSet: PARAMETER_SET,
    cases: sigGenTests.map((test, i) => ({
      tcId: test.tcId,
      // Deterministic signing (no extraEntropy) is what `Deterministic: true` means in the
      // capability registration, and it is what makes an answer key possible at all.
      signature: hex(
        ml_dsa65.sign(unhex(test.message), keypairs[i].secretKey, {
          context: unhex(test.context || ""),
        }),
      ),
    })),
  },
};

// --- sigVer -----------------------------------------------------------------
// Case 2 is tampered on purpose, so the expected answer is a mix of true/false.
const sigVerTests = keypairs.map((kp, i) => {
  const message = MESSAGES[i];
  const context = CONTEXTS[i];
  const signature = ml_dsa65.sign(unhex(message), kp.secretKey, { context: unhex(context || "") });
  const tampered = i === 1;
  if (tampered) signature[0] ^= 0x01;
  return {
    tcId: i + 1,
    pk: hex(kp.publicKey),
    message,
    context,
    signature: hex(signature),
    _expected: !tampered,
  };
});

const sigVerPack = {
  slug: "mldsa65-02-sigver",
  capability: "sig_ver",
  parameterSet: PARAMETER_SET,
  vsId: 91023,
  registration: { ...registration, mode: "sigVer", ...signatureOptions },
  prompt: prompt(
    91023,
    "sigVer",
    sigVerTests.map(({ _expected, ...test }) => test),
  ),
  answerKey: {
    vsId: 91023,
    mode: "sigVer",
    parameterSet: PARAMETER_SET,
    cases: sigVerTests.map((test) => ({ tcId: test.tcId, testPassed: test._expected })),
  },
};

// Self-check: the key must actually verify under the library before we ship it.
for (const [i, test] of sigVerTests.entries()) {
  const ok = ml_dsa65.verify(unhex(test.signature), unhex(test.message), keypairs[i].publicKey, {
    context: unhex(test.context || ""),
  });
  if (ok !== test._expected) {
    throw new Error(`sigVer tcId ${test.tcId}: expected ${test._expected}, library says ${ok}`);
  }
}

const packs = [keyGenPack, sigGenPack, sigVerPack];
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(packs, null, 2)}\n`);
console.log(
  `wrote ${packs.length} packs to ${out} (${packs.map((p) => `${p.slug}:${p.answerKey.cases.length}`).join(", ")})`,
);
