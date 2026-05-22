/**
 * i18n utilities — fbtee and grats integration.
 */
import fbt from 'fbtee';

export { fbt };

// grats generates a GraphQL schema from TypeScript decorators
export const schema = `
  type Query {
    greeting(lang: String!): String
  }
`;

export function translate(key: string, params?: Record<string, string>): string {
  // Placeholder — real usage requires babel-plugin-fbtee
  void key;
  void params;
  return key;
}