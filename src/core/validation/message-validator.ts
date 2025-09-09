import Ajv, { JSONSchemaType } from 'ajv';
import addFormats from 'ajv-formats';
import { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';

export interface ValidationResult {
  valid: boolean;
  errors?: string[];
  data?: any;
}

export interface MessageSchema {
  method: string;
  schema: JSONSchemaType<any>;
  version?: string;
  description?: string;
}

export class MessageValidator {
  private ajv: Ajv;
  private schemas = new Map<string, JSONSchemaType<any>>();
  private methodVersions = new Map<string, string[]>();

  constructor() {
    this.ajv = new Ajv({ 
      allErrors: true,
      removeAdditional: true,
      useDefaults: true,
      coerceTypes: true
    });
    
    addFormats(this.ajv);
    this.initializeStandardSchemas();
  }

  private initializeStandardSchemas(): void {
    // JSON-RPC 2.0 base schema
    this.registerSchema('jsonrpc-base', {
      type: 'object',
      required: ['jsonrpc', 'method'],
      properties: {
        jsonrpc: { type: 'string', const: '2.0' },
        method: { type: 'string', minLength: 1 },
        params: { 
          type: 'object',
          nullable: true,
          additionalProperties: true 
        },
        id: { 
          oneOf: [
            { type: 'string' },
            { type: 'number' },
            { type: 'null' }
          ]
        }
      },
      additionalProperties: false
    } as JSONSchemaType<any>);

    // MCP initialize request
    this.registerSchema('initialize', {
      type: 'object',
      required: ['jsonrpc', 'method', 'params', 'id'],
      properties: {
        jsonrpc: { type: 'string', const: '2.0' },
        method: { type: 'string', const: 'initialize' },
        params: {
          type: 'object',
          required: ['protocolVersion', 'capabilities', 'clientInfo'],
          properties: {
            protocolVersion: { type: 'string' },
            capabilities: {
              type: 'object',
              properties: {
                resources: { type: 'object', nullable: true },
                tools: { type: 'object', nullable: true },
                prompts: { type: 'object', nullable: true },
                sampling: { type: 'object', nullable: true },
                roots: { type: 'object', nullable: true }
              },
              additionalProperties: true,
              nullable: false
            },
            clientInfo: {
              type: 'object',
              required: ['name', 'version'],
              properties: {
                name: { type: 'string' },
                version: { type: 'string' }
              },
              additionalProperties: true,
              nullable: false
            }
          },
          additionalProperties: true,
          nullable: false
        },
        id: {
          oneOf: [
            { type: 'string' },
            { type: 'number' }
          ]
        }
      },
      additionalProperties: false
    } as JSONSchemaType<any>);

    // CodeGraph agent coordination message
    this.registerSchema('codegraph/agent/coordinate', {
      type: 'object',
      required: ['jsonrpc', 'method', 'params'],
      properties: {
        jsonrpc: { type: 'string', const: '2.0' },
        method: { type: 'string', const: 'codegraph/agent/coordinate' },
        params: {
          type: 'object',
          required: ['agentId', 'sessionId', 'payload'],
          properties: {
            agentId: { type: 'string', format: 'uuid' },
            sessionId: { type: 'string', format: 'uuid' },
            priority: { type: 'string', enum: ['low', 'normal', 'high', 'urgent'] },
            payload: {
              type: 'object',
              required: ['type'],
              properties: {
                type: { 
                  type: 'string',
                  enum: ['task_assignment', 'task_update', 'task_result', 'sync_request', 'heartbeat']
                },
                data: { type: 'object', additionalProperties: true, nullable: true }
              },
              additionalProperties: true,
              nullable: false
            }
          },
          additionalProperties: true,
          nullable: false
        },
        id: {
          oneOf: [
            { type: 'string' },
            { type: 'number' },
            { type: 'null' }
          ]
        }
      },
      additionalProperties: false
    } as JSONSchemaType<any>);

    // Agent registration
    this.registerSchema('session/register_agent', {
      type: 'object',
      required: ['jsonrpc', 'method', 'params'],
      properties: {
        jsonrpc: { type: 'string', const: '2.0' },
        method: { type: 'string', const: 'session/register_agent' },
        params: {
          type: 'object',
          required: ['agentId'],
          properties: {
            agentId: { type: 'string', format: 'uuid' },
            agentType: { 
              type: 'string',
              enum: ['coordinator', 'analyzer', 'transformer', 'validator', 'reporter'],
              nullable: true
            },
            capabilities: {
              type: 'array',
              items: {
                type: 'object',
                required: ['name', 'version'],
                properties: {
                  name: { type: 'string' },
                  version: { type: 'string' },
                  description: { type: 'string', nullable: true },
                  inputSchema: { type: 'object', nullable: true },
                  outputSchema: { type: 'object', nullable: true }
                },
                additionalProperties: false
              },
              nullable: true
            },
            metadata: { type: 'object', additionalProperties: true, nullable: true }
          },
          additionalProperties: false,
          nullable: false
        },
        id: {
          oneOf: [
            { type: 'string' },
            { type: 'number' }
          ]
        }
      },
      additionalProperties: false
    } as JSONSchemaType<any>);

    // Task distribution message
    this.registerSchema('codegraph/task/distribute', {
      type: 'object',
      required: ['jsonrpc', 'method', 'params'],
      properties: {
        jsonrpc: { type: 'string', const: '2.0' },
        method: { type: 'string', const: 'codegraph/task/distribute' },
        params: {
          type: 'object',
          required: ['taskId', 'targetAgents', 'payload'],
          properties: {
            taskId: { type: 'string', format: 'uuid' },
            targetAgents: {
              type: 'array',
              items: { type: 'string', format: 'uuid' },
              minItems: 1
            },
            priority: { type: 'string', enum: ['low', 'normal', 'high', 'urgent'] },
            timeout: { type: 'number', minimum: 1000, nullable: true },
            payload: {
              type: 'object',
              required: ['type', 'data'],
              properties: {
                type: { type: 'string' },
                data: { type: 'object', additionalProperties: true },
                metadata: { type: 'object', additionalProperties: true, nullable: true }
              },
              additionalProperties: false,
              nullable: false
            }
          },
          additionalProperties: false,
          nullable: false
        },
        id: {
          oneOf: [
            { type: 'string' },
            { type: 'number' }
          ]
        }
      },
      additionalProperties: false
    } as JSONSchemaType<any>);
  }

  public registerSchema(method: string, schema: JSONSchemaType<any>, version = '1.0.0'): void {
    const key = `${method}@${version}`;
    this.schemas.set(key, schema);
    
    if (!this.methodVersions.has(method)) {
      this.methodVersions.set(method, []);
    }
    this.methodVersions.get(method)!.push(version);
    
    // Also register without version for latest
    this.schemas.set(method, schema);
    
    // Compile schema for performance
    this.ajv.compile(schema);
  }

  public validateMessage(message: JsonRpcMessage, version?: string): ValidationResult {
    // First validate basic JSON-RPC structure
    const baseValidation = this.validateAgainstSchema(message, 'jsonrpc-base');
    if (!baseValidation.valid) {
      return {
        valid: false,
        errors: ['Invalid JSON-RPC format', ...(baseValidation.errors || [])]
      };
    }

    // Then validate specific method schema
    const method = message.method;
    const schemaKey = version ? `${method}@${version}` : method;
    
    if (!this.schemas.has(schemaKey)) {
      // Try to find a compatible version
      if (this.methodVersions.has(method)) {
        const versions = this.methodVersions.get(method)!;
        const latestVersion = versions[versions.length - 1];
        return this.validateMessage(message, latestVersion);
      }
      
      return {
        valid: true, // Allow unknown methods to pass through
        data: message
      };
    }

    return this.validateAgainstSchema(message, schemaKey);
  }

  public validateParams(method: string, params: any, version?: string): ValidationResult {
    const schemaKey = version ? `${method}@${version}` : method;
    const schema = this.schemas.get(schemaKey);
    
    if (!schema || !schema.properties?.params) {
      return { valid: true, data: params };
    }

    const paramsSchema = schema.properties.params as JSONSchemaType<any>;
    return this.validateAgainstSchema(params, paramsSchema);
  }

  private validateAgainstSchema(data: any, schemaKey: string | JSONSchemaType<any>): ValidationResult {
    let schema: JSONSchemaType<any>;
    
    if (typeof schemaKey === 'string') {
      schema = this.schemas.get(schemaKey);
      if (!schema) {
        return { valid: false, errors: [`Schema not found: ${schemaKey}`] };
      }
    } else {
      schema = schemaKey;
    }

    const validate = this.ajv.compile(schema);
    const valid = validate(data);

    if (!valid) {
      const errors = validate.errors?.map(error => {
        const instancePath = error.instancePath || 'root';
        const message = error.message || 'validation failed';
        return `${instancePath}: ${message}`;
      }) || ['Unknown validation error'];

      return { valid: false, errors };
    }

    return { valid: true, data };
  }

  public getSupportedMethods(): string[] {
    return Array.from(this.methodVersions.keys());
  }

  public getMethodVersions(method: string): string[] {
    return this.methodVersions.get(method) || [];
  }

  public hasMethod(method: string): boolean {
    return this.methodVersions.has(method);
  }

  public sanitizeMessage(message: JsonRpcMessage): JsonRpcMessage {
    const validation = this.validateMessage(message);
    return validation.valid ? validation.data : message;
  }

  public createErrorResponse(id: any, code: number, message: string, data?: any): JsonRpcMessage {
    return {
      jsonrpc: '2.0',
      error: {
        code,
        message,
        data
      },
      id
    };
  }

  public isValidUUID(value: string): boolean {
    const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
    return uuidRegex.test(value);
  }

  public validateAgentCapability(capability: any): ValidationResult {
    const schema: JSONSchemaType<any> = {
      type: 'object',
      required: ['name', 'version'],
      properties: {
        name: { type: 'string', minLength: 1 },
        version: { type: 'string', pattern: '^\\d+\\.\\d+\\.\\d+' },
        description: { type: 'string', nullable: true },
        inputSchema: { type: 'object', nullable: true },
        outputSchema: { type: 'object', nullable: true },
        tags: {
          type: 'array',
          items: { type: 'string' },
          nullable: true
        }
      },
      additionalProperties: false
    };

    return this.validateAgainstSchema(capability, schema);
  }

  public validateTaskPayload(payload: any): ValidationResult {
    const schema: JSONSchemaType<any> = {
      type: 'object',
      required: ['type', 'data'],
      properties: {
        type: { type: 'string', minLength: 1 },
        data: { type: 'object', additionalProperties: true },
        metadata: { type: 'object', additionalProperties: true, nullable: true },
        priority: { type: 'string', enum: ['low', 'normal', 'high', 'urgent'], nullable: true },
        timeout: { type: 'number', minimum: 1000, nullable: true },
        dependencies: {
          type: 'array',
          items: { type: 'string', format: 'uuid' },
          nullable: true
        }
      },
      additionalProperties: false
    };

    return this.validateAgainstSchema(payload, schema);
  }
}