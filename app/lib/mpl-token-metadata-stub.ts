/**
 * mpl-token-metadata-stub.ts
 *
 * Minimal hand-rolled Borsh encoder for CreateMetadataAccountV3 (instruction #33).
 *
 * Replaces @metaplex-foundation/mpl-token-metadata@2.x to eliminate the transitive
 * bigint-buffer dependency (CVE-2025-3194, no upstream patch).  Only the two symbols
 * consumed by devnet-mint-content.tsx are exported:
 *   - PROGRAM_ID
 *   - createCreateMetadataAccountV3Instruction
 *
 * The on-chain instruction layout is identical to what the Metaplex SDK emits.
 * Verified against:
 *   https://github.com/metaplex-foundation/mpl-token-metadata/blob/main/clients/js-solita/src/generated/instructions/CreateMetadataAccountV3.ts
 */

import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";

export const PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);

// ── Borsh helpers ──────────────────────────────────────────────────────────────

function writeU8(buf: number[], v: number) {
  buf.push(v & 0xff);
}

function writeU16LE(buf: number[], v: number) {
  buf.push(v & 0xff, (v >> 8) & 0xff);
}

function writeU32LE(buf: number[], v: number) {
  buf.push(
    v & 0xff,
    (v >> 8) & 0xff,
    (v >> 16) & 0xff,
    (v >> 24) & 0xff
  );
}

function writeBool(buf: number[], v: boolean) {
  buf.push(v ? 1 : 0);
}

/** Borsh string: u32 LE length prefix + UTF-8 bytes */
function writeString(buf: number[], s: string) {
  const bytes = Buffer.from(s, "utf8");
  writeU32LE(buf, bytes.length);
  for (const b of bytes) buf.push(b);
}

/** Borsh Option<T>: 0 = None, 1 = Some + value */
function writeOption<T>(
  buf: number[],
  v: T | null | undefined,
  writer: (buf: number[], v: T) => void
) {
  if (v == null) {
    writeU8(buf, 0);
  } else {
    writeU8(buf, 1);
    writer(buf, v);
  }
}

// ── DataV2 ─────────────────────────────────────────────────────────────────────

interface Creator {
  address: PublicKey;
  verified: boolean;
  share: number;
}

interface Collection {
  verified: boolean;
  key: PublicKey;
}

interface Uses {
  useMethod: number; // 0=Burn, 1=Multiple, 2=Single
  remaining: bigint;
  total: bigint;
}

interface DataV2 {
  name: string;
  symbol: string;
  uri: string;
  sellerFeeBasisPoints: number;
  creators: Creator[] | null;
  collection: Collection | null;
  uses: Uses | null;
}

function writeCreator(buf: number[], c: Creator) {
  const pk = c.address.toBytes();
  for (const b of pk) buf.push(b);
  writeBool(buf, c.verified);
  writeU8(buf, c.share);
}

function writeCollection(buf: number[], c: Collection) {
  writeBool(buf, c.verified);
  const pk = c.key.toBytes();
  for (const b of pk) buf.push(b);
}

function writeU64LE(buf: number[], v: bigint) {
  let n = v;
  for (let i = 0; i < 8; i++) {
    buf.push(Number(n & BigInt(0xff)));
    n >>= BigInt(8);
  }
}

function writeUses(buf: number[], u: Uses) {
  writeU8(buf, u.useMethod);
  writeU64LE(buf, u.remaining);
  writeU64LE(buf, u.total);
}

function writeDataV2(buf: number[], data: DataV2) {
  writeString(buf, data.name);
  writeString(buf, data.symbol);
  writeString(buf, data.uri);
  writeU16LE(buf, data.sellerFeeBasisPoints);
  writeOption(buf, data.creators, (b, cs) => {
    writeU32LE(b, cs.length);
    for (const c of cs) writeCreator(b, c);
  });
  writeOption(buf, data.collection, writeCollection);
  writeOption(buf, data.uses, writeUses);
}

// ── Instruction args / accounts ────────────────────────────────────────────────

export interface CreateMetadataAccountArgsV3 {
  data: DataV2;
  isMutable: boolean;
  collectionDetails: null; // only None supported (sufficient for devnet mints)
}

export interface CreateMetadataAccountV3Accounts {
  metadata: PublicKey;
  mint: PublicKey;
  mintAuthority: PublicKey;
  payer: PublicKey;
  updateAuthority: PublicKey;
  systemProgram?: PublicKey;
  rent?: PublicKey;
}

export interface CreateMetadataAccountV3InstructionArgs {
  createMetadataAccountArgsV3: CreateMetadataAccountArgsV3;
}

/** Instruction discriminator = 33 (matches Metaplex on-chain program) */
const DISCRIMINATOR = 33;

export function createCreateMetadataAccountV3Instruction(
  accounts: CreateMetadataAccountV3Accounts,
  args: CreateMetadataAccountV3InstructionArgs,
  programId: PublicKey = PROGRAM_ID
): TransactionInstruction {
  const { createMetadataAccountArgsV3: a } = args;

  const buf: number[] = [];
  writeU8(buf, DISCRIMINATOR);
  writeDataV2(buf, a.data);
  writeBool(buf, a.isMutable);
  // collectionDetails: Option<CollectionDetails> — always None
  writeU8(buf, 0);

  const keys = [
    { pubkey: accounts.metadata, isWritable: true, isSigner: false },
    { pubkey: accounts.mint, isWritable: false, isSigner: false },
    { pubkey: accounts.mintAuthority, isWritable: false, isSigner: true },
    { pubkey: accounts.payer, isWritable: true, isSigner: true },
    { pubkey: accounts.updateAuthority, isWritable: false, isSigner: false },
    {
      pubkey: accounts.systemProgram ?? SystemProgram.programId,
      isWritable: false,
      isSigner: false,
    },
  ];

  if (accounts.rent != null) {
    keys.push({ pubkey: accounts.rent, isWritable: false, isSigner: false });
  }

  return new TransactionInstruction({
    programId,
    keys,
    data: Buffer.from(buf),
  });
}
