My implementation involves the creation of a new socket for every http request. The crawler has a list of visited urls and recursively steps each page.

I have a custom builder type called RequestBuilder that serializes requests and deserializes responses
