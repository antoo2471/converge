import {transform} from '@babel/standalone';
/** Compile syntax only. Evaluation happens in the sandbox document, never here. */
export function compileArtifact(source:string):string{
 if(source.length>2*1024*1024)throw Error('Artifact trop volumineux');
 const output=transform(source,{
  plugins:[({types:t}:any)=>({visitor:{
   ImportDeclaration(path:any){
    const source=path.node.source.value;
    if(!['react','react-dom','react-dom/client'].includes(source))throw path.buildCodeFrameError('Le runtime isolé accepte uniquement React et ReactDOM, disponibles hors ligne.');
    const root=t.memberExpression(t.identifier('window'),t.identifier(source==='react'?'React':'ReactDOM'));
    const bindings=path.node.specifiers.map((specifier:any)=>t.variableDeclarator(t.identifier(specifier.local.name),specifier.type==='ImportSpecifier'?t.memberExpression(root,t.identifier(specifier.imported.name)):root));
    if(bindings.length)path.replaceWith(t.variableDeclaration('const',bindings));else path.remove();
   },
   ExportDefaultDeclaration(path:any){path.replaceWith(t.expressionStatement(t.assignmentExpression('=',t.memberExpression(t.identifier('window'),t.identifier('ArtifactApp')),t.toExpression(path.node.declaration))));},
   ExportNamedDeclaration(path:any){if(path.node.declaration)path.replaceWith(path.node.declaration);else path.remove();}
  }})],
  presets:[['react',{runtime:'classic'}],['typescript',{}]],filename:'artifact.tsx'
 }).code;
 return `(()=>{${output}\n;window.ReactDOM.createRoot(document.getElementById('app')).render(window.React.createElement(window.ArtifactApp || (typeof App!=='undefined'?App:()=>window.React.createElement('p',null,'Exportez un composant React par défaut.'))));})()`;
}
