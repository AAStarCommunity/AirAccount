/**
 * WebAuthn 错误处理系统
 * 提供全面的错误分类和处理机制，与Rust CA保持一致
 */

export enum WebAuthnErrorType {
  // 用户错误 (User Errors)
  USER_NOT_FOUND = 'USER_NOT_FOUND',
  NO_DEVICES_REGISTERED = 'NO_DEVICES_REGISTERED',
  DEVICE_NOT_FOUND = 'DEVICE_NOT_FOUND',
  INVALID_USER_INPUT = 'INVALID_USER_INPUT',
  USER_VERIFICATION_REQUIRED = 'USER_VERIFICATION_REQUIRED',

  // 安全错误 (Security Errors)
  CHALLENGE_VERIFICATION_FAILED = 'CHALLENGE_VERIFICATION_FAILED',
  SIGNATURE_VERIFICATION_FAILED = 'SIGNATURE_VERIFICATION_FAILED',
  COUNTER_ROLLBACK = 'COUNTER_ROLLBACK',
  ORIGIN_MISMATCH = 'ORIGIN_MISMATCH',
  RP_ID_MISMATCH = 'RP_ID_MISMATCH',
  ATTESTATION_VERIFICATION_FAILED = 'ATTESTATION_VERIFICATION_FAILED',

  // 系统错误 (System Errors)
  DATABASE_ERROR = 'DATABASE_ERROR',
  ENCODING_ERROR = 'ENCODING_ERROR',
  NETWORK_ERROR = 'NETWORK_ERROR',
  INVALID_STATE = 'INVALID_STATE',
  TIMEOUT = 'TIMEOUT',
  INTERNAL_ERROR = 'INTERNAL_ERROR',

  // 配置错误 (Configuration Errors)
  INVALID_CONFIG = 'INVALID_CONFIG',
  UNSUPPORTED_ALGORITHM = 'UNSUPPORTED_ALGORITHM',
  UNSUPPORTED_TRANSPORT = 'UNSUPPORTED_TRANSPORT',

  // 协议错误 (Protocol Errors)
  INVALID_RESPONSE_FORMAT = 'INVALID_RESPONSE_FORMAT',
  MISSING_REQUIRED_FIELD = 'MISSING_REQUIRED_FIELD',
  INVALID_CREDENTIAL_TYPE = 'INVALID_CREDENTIAL_TYPE',
  CREDENTIAL_EXCLUDED = 'CREDENTIAL_EXCLUDED',
  
  // 业务逻辑错误 (Business Logic Errors)
  REGISTRATION_IN_PROGRESS = 'REGISTRATION_IN_PROGRESS',
  AUTHENTICATION_IN_PROGRESS = 'AUTHENTICATION_IN_PROGRESS',
  DEVICE_ALREADY_REGISTERED = 'DEVICE_ALREADY_REGISTERED',
  SESSION_EXPIRED = 'SESSION_EXPIRED',
  INVALID_SESSION = 'INVALID_SESSION',
}

export interface WebAuthnErrorDetails {
  type: WebAuthnErrorType;
  message: string;
  code: string;
  statusCode: number;
  category: 'user' | 'security' | 'system' | 'config' | 'protocol' | 'business';
  retryable: boolean;
  context?: Record<string, any>;
}

export class WebAuthnError extends Error {
  public readonly type: WebAuthnErrorType;
  public readonly code: string;
  public readonly statusCode: number;
  public readonly category: string;
  public readonly retryable: boolean;
  public readonly context?: Record<string, any>;

  constructor(details: WebAuthnErrorDetails) {
    super(details.message);
    this.name = 'WebAuthnError';
    this.type = details.type;
    this.code = details.code;
    this.statusCode = details.statusCode;
    this.category = details.category;
    this.retryable = details.retryable;
    this.context = details.context;
  }

  static userNotFound(userId: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.USER_NOT_FOUND,
      message: `User not found: ${userId}`,
      code: 'WEBAUTHN_USER_NOT_FOUND',
      statusCode: 404,
      category: 'user',
      retryable: false,
      context: { userId }
    });
  }

  static noDevicesRegistered(userId: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.NO_DEVICES_REGISTERED,
      message: `No devices registered for user: ${userId}`,
      code: 'WEBAUTHN_NO_DEVICES',
      statusCode: 400,
      category: 'user',
      retryable: false,
      context: { userId }
    });
  }

  static deviceNotFound(credentialId: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.DEVICE_NOT_FOUND,
      message: `Authenticator device not found`,
      code: 'WEBAUTHN_DEVICE_NOT_FOUND',
      statusCode: 404,
      category: 'user',
      retryable: false,
      context: { credentialId: credentialId.substring(0, 16) + '...' }
    });
  }

  static challengeVerificationFailed(challenge?: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.CHALLENGE_VERIFICATION_FAILED,
      message: 'Challenge verification failed - invalid or expired challenge',
      code: 'WEBAUTHN_CHALLENGE_FAILED',
      statusCode: 401,
      category: 'security',
      retryable: false,
      context: { challenge: challenge?.substring(0, 16) + '...' }
    });
  }

  static signatureVerificationFailed(): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.SIGNATURE_VERIFICATION_FAILED,
      message: 'Signature verification failed',
      code: 'WEBAUTHN_SIGNATURE_FAILED',
      statusCode: 401,
      category: 'security',
      retryable: false
    });
  }

  static counterRollback(currentCounter: number, receivedCounter: number): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.COUNTER_ROLLBACK,
      message: 'Counter rollback detected - possible replay attack',
      code: 'WEBAUTHN_COUNTER_ROLLBACK',
      statusCode: 401,
      category: 'security',
      retryable: false,
      context: { currentCounter, receivedCounter }
    });
  }

  static originMismatch(expected: string, received: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.ORIGIN_MISMATCH,
      message: 'Origin mismatch',
      code: 'WEBAUTHN_ORIGIN_MISMATCH',
      statusCode: 401,
      category: 'security',
      retryable: false,
      context: { expected, received }
    });
  }

  static rpIdMismatch(expected: string, received: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.RP_ID_MISMATCH,
      message: 'Relying Party ID mismatch',
      code: 'WEBAUTHN_RP_ID_MISMATCH',
      statusCode: 401,
      category: 'security',
      retryable: false,
      context: { expected, received }
    });
  }

  static databaseError(operation: string, cause?: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.DATABASE_ERROR,
      message: `Database operation failed: ${operation}`,
      code: 'WEBAUTHN_DATABASE_ERROR',
      statusCode: 500,
      category: 'system',
      retryable: true,
      context: { operation, cause }
    });
  }

  static invalidState(expectedState: string, currentState: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.INVALID_STATE,
      message: `Invalid state transition from ${currentState} to ${expectedState}`,
      code: 'WEBAUTHN_INVALID_STATE',
      statusCode: 400,
      category: 'business',
      retryable: false,
      context: { expectedState, currentState }
    });
  }

  static invalidResponseFormat(field: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.INVALID_RESPONSE_FORMAT,
      message: `Invalid response format: missing or invalid ${field}`,
      code: 'WEBAUTHN_INVALID_FORMAT',
      statusCode: 400,
      category: 'protocol',
      retryable: false,
      context: { field }
    });
  }

  static sessionExpired(sessionId: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.SESSION_EXPIRED,
      message: 'Session has expired',
      code: 'WEBAUTHN_SESSION_EXPIRED',
      statusCode: 401,
      category: 'business',
      retryable: false,
      context: { sessionId: sessionId.substring(0, 8) + '...' }
    });
  }

  static registrationInProgress(userId: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.REGISTRATION_IN_PROGRESS,
      message: 'Registration already in progress for this user',
      code: 'WEBAUTHN_REGISTRATION_IN_PROGRESS',
      statusCode: 409,
      category: 'business',
      retryable: true,
      context: { userId }
    });
  }

  static authenticationInProgress(userId?: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.AUTHENTICATION_IN_PROGRESS,
      message: 'Authentication already in progress',
      code: 'WEBAUTHN_AUTHENTICATION_IN_PROGRESS',
      statusCode: 409,
      category: 'business',
      retryable: true,
      context: { userId }
    });
  }

  static deviceAlreadyRegistered(credentialId: string): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.DEVICE_ALREADY_REGISTERED,
      message: 'Device is already registered',
      code: 'WEBAUTHN_DEVICE_ALREADY_REGISTERED',
      statusCode: 409,
      category: 'business',
      retryable: false,
      context: { credentialId: credentialId.substring(0, 16) + '...' }
    });
  }

  static timeout(operation: string, timeoutMs: number): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.TIMEOUT,
      message: `Operation timed out: ${operation}`,
      code: 'WEBAUTHN_TIMEOUT',
      statusCode: 408,
      category: 'system',
      retryable: true,
      context: { operation, timeoutMs }
    });
  }

  static internalError(message: string, cause?: any): WebAuthnError {
    return new WebAuthnError({
      type: WebAuthnErrorType.INTERNAL_ERROR,
      message: `Internal error: ${message}`,
      code: 'WEBAUTHN_INTERNAL_ERROR',
      statusCode: 500,
      category: 'system',
      retryable: false,
      context: { cause: cause?.toString() }
    });
  }

  toJSON() {
    return {
      error: {
        type: this.type,
        message: this.message,
        code: this.code,
        statusCode: this.statusCode,
        category: this.category,
        retryable: this.retryable,
        context: this.context,
      }
    };
  }
}

/**
 * 错误处理中间件 - 用于Express路由
 */
export function handleWebAuthnError(error: any): WebAuthnError {
  if (error instanceof WebAuthnError) {
    return error;
  }

  // 转换@simplewebauthn错误
  if (error.name === 'VerificationError') {
    return WebAuthnError.signatureVerificationFailed();
  }

  // 转换数据库错误
  if (error.code === 'SQLITE_ERROR' || error.code?.startsWith('SQLITE_')) {
    return WebAuthnError.databaseError('sqlite operation', error.message);
  }

  // 转换网络错误
  if (error.code === 'ECONNREFUSED' || error.code === 'ETIMEDOUT') {
    return WebAuthnError.internalError('Network error', error.message);
  }

  // 默认内部错误
  return WebAuthnError.internalError(error.message || 'Unknown error', error);
}