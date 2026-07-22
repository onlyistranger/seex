export type ValidationCode = "required" | "invalid_lcsc_id" | "invalid_color";

export interface ValidationResult {
  valid: boolean;
  value: string;
  normalized: string | null;
  code?: ValidationCode;
}

const LCSC_ID_PATTERN = /^[cC]\d+$/;
const HEX_COLOR_PATTERN = /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

export function validateLcscId(value: string): ValidationResult {
  const trimmed = value.trim();
  const valid = LCSC_ID_PATTERN.test(trimmed);
  return {
    valid,
    value: trimmed,
    normalized: valid ? trimmed.toUpperCase() : null,
    ...(valid ? {} : { code: "invalid_lcsc_id" as const }),
  };
}

export function validateHexColor(value: string): ValidationResult {
  const trimmed = value.trim();
  if (!trimmed) {
    return { valid: true, value: trimmed, normalized: null };
  }

  const match = HEX_COLOR_PATTERN.exec(trimmed);
  return {
    valid: match !== null,
    value: trimmed,
    normalized: match ? `#${match[1].toUpperCase()}` : null,
    ...(match ? {} : { code: "invalid_color" as const }),
  };
}

export function validateRequiredPath(value: string): ValidationResult {
  const trimmed = value.trim();
  return {
    valid: trimmed.length > 0,
    value: trimmed,
    normalized: trimmed || null,
    ...(trimmed ? {} : { code: "required" as const }),
  };
}
