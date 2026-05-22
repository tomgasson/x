/**
 * Graph utilities — d3 and relay integration.
 */
import * as d3 from 'd3';
import { fetchQuery, RelayEnvironment } from 'relay-runtime';

// Re-export d3 for convenience
export { d3 };

// Placeholder — wire up a real Relay environment in production
export interface GraphConfig {
  width: number;
  height: number;
  container: HTMLElement;
}

export function initGraph(config: GraphConfig): void {
  const svg = d3.select(config.container)
    .append('svg')
    .attr('width', config.width)
    .attr('height', config.height);
  // TODO: wire up rendering
  void svg;
  void fetchQuery;
}