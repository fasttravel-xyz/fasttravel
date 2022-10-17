**fasttravel-rt-client**

This package provides the client-sdk for integrating fasttravel-rt realtime features into an application.

> **WARNING:** Currently, this sdk is used for local development and testing, tenant management is not implemented yet, so deployment not possible.


**fasttravel-rt** provides only the realtime components, so it depends on few additional services for it's functioning. fasttravel-rt doesn't handle user authetication and authorization, so it is upto you to use an auth-provider of your choice and provide the client-sdk an access-token during the client creation process. For this you have to provide fasttravel-rt the PUBLIC_KEY of your auth-provider so that fasttravel-rt could decode the access-token. As fasttravel-rt is in early stage of development tenant management admin-console is not provided, so deplyment is not possible yet. Though it is possible to run in single tenant configuration by providing the details to the server. 


Below are the steps to create a client using the client-sdk that uses Auth0 as the example auth-provider (Any other auth-provider will work, the app needs to provide an access-token with the realtime:api audience, and token.sub as the tenant.user):

* Provide fasttravel_rt with the `public_key_of_your_auth_provider` (only once during initial setup once tenant management is available).
* Create the `FasttravelClient` with `public_client_id` and `public_client_api_key` received during initial setup.
* Authenticate tenant user using Auth0 authetication flow (login).
* During Auth0 authentication request api-access with audience "fasttravel:realtime:api".
* Provide that access-token to the FasttravelClient.
* FasttravelClient uses the access-token to generate internal tickets for the user (token.sub).
* The `public_client_api_key` and the `access_token_from_auth_provider` allows fasttravel to authorize (roles, scopes, permissions) the realtime requests.
* **NOTE**: If your app uses multiple apis along with fasttravel:realtime:api you should get multiple access-tokens for each APIs authorization (in case of Auth0, make the WebAuth.checksession(audience, scope) call on an already autheticated user with each API audience and get a different access-token.) For fasstravel:realtime:api always generate a separate access-token and don't provide a token with other audiences to FasttravelClient. Remember these are `"Bearer"` tokens you are generating.

```js
// Example Auth0 Access Token with the "fasttravel:realtime:api" audience added:

{
  "iss": "https://my-tenant-domain.auth0.com/",
  "sub": "auth0|123456",
  "aud": [
    "fasttravel:realtime:api",
    "https://my-domain.auth0.com/userinfo",
  ],
  "azp": "my_client_id",
  "exp": 1311281970,
  "iat": 1311280970,
  "scope": "openid profile read:realtime read:admin"
}

// [REF]: https://auth0.com/docs/secure/tokens/access-tokens/get-access-tokens
// [REF]: https://auth0.com/docs/libraries/auth0js#using-checksession-to-acquire-new-tokens

```