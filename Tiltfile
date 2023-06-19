
# Usage default features:
# tilt up
#
# Usage with features:
# tilt up telemetry
config.define_string("features", args=True)
cfg = config.parse()
features = cfg.get('features', "")
print("compiling with features: {}".format(features))

local_resource('compile', 'just compile %s' % features)
docker_build('21kyu/agent', '.', dockerfile='Dockerfile')
k8s_yaml('yaml/crd.yaml')
k8s_yaml('yaml/deploy.yaml')
k8s_resource('agent', port_forwards=8080)