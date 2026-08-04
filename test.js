
const deepVariableReplacer = (node, variables) => {
  if (typeof node === 'string') {
    let result = node;
    for (const [key, value] of Object.entries(variables)) {
      result = result.replace(new RegExp(\\\\\{\\\\{$\{key}\\\\}\\\\}\, 'g'), value);
    }
    return result;
  }
  if (Array.isArray(node)) {
    return node.map((item) => deepVariableReplacer(item, variables));
  }
  if (node && typeof node === 'object') {
    const newNode = {};
    for (const key in node) {
      newNode[key] = deepVariableReplacer(node[key], variables);
    }
    return newNode;
  }
  return node;
};

const curlJson = {
  header: {
    'Authorization': 'Bearer {{API_KEY}}',
    'Content-Type': 'application/json'
  }
};
const allVariables = { API_KEY: 'key2' };

const headers = deepVariableReplacer(curlJson.header || {}, allVariables);
console.log(headers);

