Running Server
========
`cargo run --bin server`

Serving Typescript Client
========
Before starting server!

First generate Typescript bindings
`cargo test`

Next compile Typescript
`cd ts; deno bundle --config tsconfig.json client.ts static/client.js`

Finally run server with instructions above
